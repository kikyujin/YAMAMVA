use std::fs;
use std::path::Path;
use tempfile::TempDir;

use yamamva::parser::{parse, parse_world, parse_file_scene_ref};
use yamamva::engine::{Engine, ExecArgs};
use yamamva::registry::{Registry, YAMAMVA_END, YAMAMVA_PASS, YAMAMVA_BLOCKING};
use yamamva::save;
use yamamva::ffi;

const CMD_BG: i32 = 1;
const CMD_TEXT: i32 = 2;
const CMD_SPEAKER: i32 = 3;
const CMD_MENU: i32 = 4;

fn setup_world(dir: &Path, world_yaml: &str, scene_files: &[(&str, &str)]) {
    fs::write(dir.join("world.yaml"), world_yaml).unwrap();
    let scenes_dir = dir.join("scenes");
    fs::create_dir_all(&scenes_dir).unwrap();
    for (name, content) in scene_files {
        fs::write(scenes_dir.join(name), content).unwrap();
    }
}

fn make_registry() -> Registry {
    let mut r = Registry::new();
    r.register("bg", CMD_BG, YAMAMVA_PASS);
    r.register("text", CMD_TEXT, YAMAMVA_PASS);
    r.register("speaker", CMD_SPEAKER, YAMAMVA_PASS);
    r.register("hearingmenu", CMD_MENU, YAMAMVA_BLOCKING);
    r
}

// --- parse_file_scene_ref unit tests ---

#[test]
fn test_parse_file_scene_ref_with_colon() {
    let (file, scene) = parse_file_scene_ref("intro:scene_intro").unwrap();
    assert_eq!(file, Some("intro".to_string()));
    assert_eq!(scene, "scene_intro");
}

#[test]
fn test_parse_file_scene_ref_without_colon() {
    let (file, scene) = parse_file_scene_ref("scene_intro").unwrap();
    assert_eq!(file, None);
    assert_eq!(scene, "scene_intro");
}

#[test]
fn test_parse_file_scene_ref_empty_scene_error() {
    assert!(parse_file_scene_ref("intro:").is_err());
}

// --- T1: world.yaml + 2 scene files, basic load and exec ---

#[test]
fn test_load_world_basic() {
    let dir = TempDir::new().unwrap();
    setup_world(dir.path(),
        r#"
id: test_world
title: Test World
entry: intro:scene_intro
scene_path: scenes/
state:
  count: 0
characters:
  npc_a: { name: "NPC-A" }
"#,
        &[
            ("intro.yaml", r#"
scene_intro:
  - text: "Welcome"
  - jump:
      - next: main:scene_main
"#),
            ("main.yaml", r#"
scene_main:
  - text: "Hello"
  - end: true
"#),
        ],
    );

    let scenario = parse_world(&dir.path().join("world.yaml")).unwrap();
    assert_eq!(scenario.id, "test_world");
    assert_eq!(scenario.entry, "intro:scene_intro");
    assert!(scenario.scenes.contains_key("scene_intro"));
    assert!(scenario.scenes.contains_key("scene_main"));
    assert_eq!(scenario.scene_file["scene_intro"], "intro");
    assert_eq!(scenario.scene_file["scene_main"], "main");
    assert_eq!(scenario.scene_path, Some("scenes/".to_string()));

    let mut engine = Engine::new(scenario, make_registry());
    let mut args = ExecArgs::new();

    assert_eq!(engine.exec(&mut args), CMD_TEXT); // "Welcome"
    // jump to main:scene_main
    assert_eq!(engine.exec(&mut args), CMD_TEXT); // "Hello"
    assert_eq!(engine.exec(&mut args), YAMAMVA_END);
}

// --- T2: file:scene jump resolves correctly ---

#[test]
fn test_file_scene_jump() {
    let dir = TempDir::new().unwrap();
    setup_world(dir.path(),
        r#"
id: test
title: test
entry: a:scene_a
scene_path: scenes/
state: {}
"#,
        &[
            ("a.yaml", r#"
scene_a:
  - text: "in A"
  - jump:
      - next: b:scene_b
"#),
            ("b.yaml", r#"
scene_b:
  - text: "in B"
  - end: true
"#),
        ],
    );

    let scenario = parse_world(&dir.path().join("world.yaml")).unwrap();
    let mut engine = Engine::new(scenario, make_registry());
    let mut args = ExecArgs::new();

    assert_eq!(engine.exec(&mut args), CMD_TEXT);
    assert_eq!(engine.current_file(), "a");
    assert_eq!(engine.exec(&mut args), CMD_TEXT); // jumped to b
    assert_eq!(engine.current_file(), "b");
    assert_eq!(engine.exec(&mut args), YAMAMVA_END);
}

// --- T3: no-colon reference succeeds within same file ---

#[test]
fn test_same_file_jump() {
    let dir = TempDir::new().unwrap();
    setup_world(dir.path(),
        r#"
id: test
title: test
entry: multi:scene_first
scene_path: scenes/
state: {}
"#,
        &[
            ("multi.yaml", r#"
scene_first:
  - text: "first"
  - jump:
      - next: scene_second
scene_second:
  - text: "second"
  - end: true
"#),
        ],
    );

    let scenario = parse_world(&dir.path().join("world.yaml")).unwrap();
    let mut engine = Engine::new(scenario, make_registry());
    let mut args = ExecArgs::new();

    assert_eq!(engine.exec(&mut args), CMD_TEXT); // first
    assert_eq!(engine.exec(&mut args), CMD_TEXT); // second (same file jump)
    assert_eq!(engine.current_file(), "multi");
    assert_eq!(engine.exec(&mut args), YAMAMVA_END);
}

// --- T4: no-colon reference to another file's scene → error (warning) ---

#[test]
fn test_cross_file_without_colon_error() {
    let dir = TempDir::new().unwrap();
    setup_world(dir.path(),
        r#"
id: test
title: test
entry: a:scene_a
scene_path: scenes/
state: {}
"#,
        &[
            ("a.yaml", r#"
scene_a:
  - text: "in A"
  - jump:
      - next: scene_b
"#),
            ("b.yaml", r#"
scene_b:
  - text: "in B"
  - end: true
"#),
        ],
    );

    let scenario = parse_world(&dir.path().join("world.yaml")).unwrap();
    let mut engine = Engine::new(scenario, make_registry());
    let mut args = ExecArgs::new();

    assert_eq!(engine.exec(&mut args), CMD_TEXT); // "in A"
    // jump to scene_b without colon → should produce warning and END
    assert_eq!(engine.exec(&mut args), YAMAMVA_END);
}

// --- T5: entry with invalid scene → parse error ---

#[test]
fn test_invalid_entry_error() {
    let dir = TempDir::new().unwrap();
    setup_world(dir.path(),
        r#"
id: test
title: test
entry: intro:scene_nonexistent
scene_path: scenes/
state: {}
"#,
        &[
            ("intro.yaml", r#"
scene_intro:
  - end: true
"#),
        ],
    );

    let result = parse_world(&dir.path().join("world.yaml"));
    assert!(result.is_err());
    let err = result.unwrap_err().to_string();
    assert!(err.contains("scene_nonexistent"), "error should mention the missing scene: {}", err);
}

// --- T6: v1.0 compat — single YAML load still works ---

#[test]
fn test_v10_compat() {
    let yaml = r#"
id: test_v10
title: v10 test
entry: scene_start
scenes:
  scene_start:
    - text: "Hello"
    - jump:
        - next: scene_end
  scene_end:
    - text: "Bye"
    - end: true
"#;
    let scenario = parse(yaml).unwrap();
    assert_eq!(scenario.scene_file["scene_start"], "__root__");
    assert_eq!(scenario.scene_file["scene_end"], "__root__");
    assert!(scenario.scene_path.is_none());

    let mut engine = Engine::new(scenario, make_registry());
    let mut args = ExecArgs::new();
    assert_eq!(engine.current_file(), "__root__");
    assert_eq!(engine.exec(&mut args), CMD_TEXT);
    assert_eq!(engine.exec(&mut args), CMD_TEXT);
    assert_eq!(engine.exec(&mut args), YAMAMVA_END);
}

// --- T7: save/restore preserves current_file ---

#[test]
fn test_save_restore_with_world() {
    let dir = TempDir::new().unwrap();
    setup_world(dir.path(),
        r#"
id: test
title: test
entry: a:scene_a
scene_path: scenes/
state:
  x: 0
"#,
        &[
            ("a.yaml", r#"
scene_a:
  - text: "A"
  - jump:
      - next: b:scene_b
"#),
            ("b.yaml", r#"
scene_b:
  - do:
      x: "x + 1"
  - text: "B1"
  - text: "B2"
  - end: true
"#),
        ],
    );

    let scenario = parse_world(&dir.path().join("world.yaml")).unwrap();
    let mut engine = Engine::new(scenario.clone(), make_registry());
    let mut args = ExecArgs::new();

    engine.exec(&mut args); // text "A"
    engine.exec(&mut args); // text "B1" (jumped to b, do processed)

    assert_eq!(engine.current_file(), "b");

    let save_json = save::save(&engine);
    let saved: serde_json::Value = serde_json::from_str(&save_json).unwrap();
    assert_eq!(saved["current_file"], "b");

    let mut engine2 = save::restore(scenario, make_registry(), &save_json).unwrap();
    assert_eq!(engine2.current_file(), "b");

    let mut args2 = ExecArgs::new();
    assert_eq!(engine2.exec(&mut args2), CMD_TEXT); // "B2"
    assert_eq!(engine2.exec(&mut args2), YAMAMVA_END);
}

// --- T8: world.yaml with inline scenes (no scene files) ---

#[test]
fn test_world_inline_scenes() {
    let dir = TempDir::new().unwrap();
    fs::write(dir.path().join("world.yaml"), r#"
id: test_inline
title: test
entry: scene_hello
state: {}
scenes:
  scene_hello:
    - text: "inline hello"
    - end: true
"#).unwrap();

    let scenario = parse_world(&dir.path().join("world.yaml")).unwrap();
    assert_eq!(scenario.scene_file["scene_hello"], "__world__");

    let mut engine = Engine::new(scenario, make_registry());
    let mut args = ExecArgs::new();
    assert_eq!(engine.exec(&mut args), CMD_TEXT);
    assert_eq!(engine.exec(&mut args), YAMAMVA_END);
}

// --- T9: incase with file:scene notation ---

#[test]
fn test_incase_file_scene() {
    let dir = TempDir::new().unwrap();
    setup_world(dir.path(),
        r#"
id: test
title: test
entry: hub:scene_hub
scene_path: scenes/
state: {}
"#,
        &[
            ("hub.yaml", r#"
scene_hub:
  - hearingmenu:
      elements:
        - { key: go, label: "Go" }
  - incase:
      - when: "$result == 'go'"
        next: dest:scene_dest
      - next: hub:scene_hub
"#),
            ("dest.yaml", r#"
scene_dest:
  - text: "arrived"
  - end: true
"#),
        ],
    );

    let scenario = parse_world(&dir.path().join("world.yaml")).unwrap();
    let mut engine = Engine::new(scenario, make_registry());
    let mut args = ExecArgs::new();

    assert_eq!(engine.exec(&mut args), CMD_MENU); // hearingmenu
    args.result = Some("go".to_string());
    assert_eq!(engine.exec(&mut args), CMD_TEXT); // "arrived" (incase → dest:scene_dest)
    assert_eq!(engine.current_file(), "dest");
    assert_eq!(engine.exec(&mut args), YAMAMVA_END);
}

// --- T10: duplicate scene across files → parse error ---

#[test]
fn test_duplicate_scene_error() {
    let dir = TempDir::new().unwrap();
    setup_world(dir.path(),
        r#"
id: test
title: test
entry: a:scene_dup
scene_path: scenes/
state: {}
"#,
        &[
            ("a.yaml", r#"
scene_dup:
  - end: true
"#),
            ("b.yaml", r#"
scene_dup:
  - end: true
"#),
        ],
    );

    let result = parse_world(&dir.path().join("world.yaml"));
    assert!(result.is_err());
    let err = result.unwrap_err().to_string();
    assert!(err.contains("scene_dup"), "error should mention the duplicate scene: {}", err);
}

// --- F1: yamamva_load_world FFI test ---

#[test]
fn test_ffi_load_world() {
    use std::ffi::CString;

    let dir = TempDir::new().unwrap();
    setup_world(dir.path(),
        r#"
id: ffi_test
title: FFI Test
entry: a:scene_a
scene_path: scenes/
state: {}
"#,
        &[
            ("a.yaml", r#"
scene_a:
  - text: "hello from world"
  - end: true
"#),
        ],
    );

    let path = CString::new(dir.path().join("world.yaml").to_str().unwrap()).unwrap();
    let h = unsafe { ffi::yamamva_load_world(path.as_ptr()) };
    assert!(!h.is_null());

    unsafe {
        let nt = CString::new("text").unwrap();
        ffi::yamamva_register(h, nt.as_ptr(), CMD_TEXT, YAMAMVA_PASS);

        let mut args = std::mem::zeroed::<ffi::FfiArgs>();
        let cmd = ffi::yamamva_exec(h, &mut args);
        assert_eq!(cmd, CMD_TEXT);

        let cmd = ffi::yamamva_exec(h, &mut args);
        assert_eq!(cmd, YAMAMVA_END);

        ffi::yamamva_free(h);
    }
}

// --- F2: yamamva_load_world with invalid path → NULL ---

#[test]
fn test_ffi_load_world_invalid_path() {
    use std::ffi::CString;

    let path = CString::new("/nonexistent/world.yaml").unwrap();
    let h = unsafe { ffi::yamamva_load_world(path.as_ptr()) };
    assert!(h.is_null());
}
