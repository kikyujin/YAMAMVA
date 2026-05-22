use crate::engine::{Cursor, Engine};
use crate::registry::Registry;
use crate::scenario::Scenario;
use crate::state::State;

pub fn save(engine: &Engine) -> String {
    let result_val = match engine.state().get_result() {
        Some(r) => serde_json::Value::String(r.clone()),
        None => serde_json::Value::Null,
    };

    let json = serde_json::json!({
        "version": 1,
        "scene_id": engine.cursor().scene_id,
        "node_index": engine.cursor().node_index,
        "state": engine.state().dump(),
        "result": result_val,
    });

    serde_json::to_string(&json).unwrap_or_else(|_| "{}".to_string())
}

pub fn restore(scenario: Scenario, registry: Registry, save_json: &str) -> Result<Engine, String> {
    let save: serde_json::Value = serde_json::from_str(save_json)
        .map_err(|e| format!("invalid save JSON: {}", e))?;

    let scene_id = save.get("scene_id")
        .and_then(|v| v.as_str())
        .ok_or("missing scene_id in save")?
        .to_string();

    let node_index = save.get("node_index")
        .and_then(|v| v.as_u64())
        .ok_or("missing node_index in save")? as usize;

    let mut state = State::from_json_map(&scenario.initial_state);

    if let Some(state_json) = save.get("state") {
        state.restore(state_json);
    }

    if let Some(result_val) = save.get("result")
        && let Some(r) = result_val.as_str() {
            state.set_result(Some(r.to_string()));
        }

    Ok(Engine {
        scenario,
        state,
        registry,
        cursor: Cursor {
            scene_id,
            node_index,
        },
        waiting_result: false,
        warnings: Vec::new(),
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::engine::ExecArgs;
    use crate::parser::parse;
    use crate::registry::{YAMAMVA_PASS, YAMAMVA_BLOCKING, YAMAMVA_END};
    use crate::state::Value;

    const CMD_BG: i32 = 1;
    const CMD_TEXT: i32 = 2;
    const CMD_SPEAKER: i32 = 3;
    const CMD_MENU: i32 = 4;

    fn make_engine(yaml: &str) -> Engine {
        let scenario = parse(yaml).unwrap();
        let mut registry = Registry::new();
        registry.register("bg", CMD_BG, YAMAMVA_PASS);
        registry.register("text", CMD_TEXT, YAMAMVA_PASS);
        registry.register("speaker", CMD_SPEAKER, YAMAMVA_PASS);
        registry.register("hearingmenu", CMD_MENU, YAMAMVA_BLOCKING);
        Engine::new(scenario, registry)
    }

    #[test]
    fn test_save_and_restore() {
        let yaml = r#"
id: test
title: test
entry: scene_a
state:
  heard_elmar: false
  hearing_count: 0
scenes:
  scene_a:
    - bg: lobby
    - text: "Part 1"
    - do:
        heard_elmar: true
        hearing_count: "hearing_count + 1"
    - text: "Part 2"
    - end: true
"#;
        let mut engine = make_engine(yaml);
        let mut args = ExecArgs::new();

        assert_eq!(engine.exec(&mut args), CMD_BG);
        assert_eq!(engine.exec(&mut args), CMD_TEXT); // "Part 1"
        // exec again: do is processed internally, then "Part 2" is returned
        assert_eq!(engine.exec(&mut args), CMD_TEXT); // "Part 2"

        // Save here — after do has been processed
        let save_json = save(&engine);

        // Verify save contents
        let saved: serde_json::Value = serde_json::from_str(&save_json).unwrap();
        assert_eq!(saved["scene_id"], "scene_a");
        assert_eq!(saved["state"]["heard_elmar"], true);
        assert_eq!(saved["state"]["hearing_count"], 1);

        // Restore into a new engine
        let scenario2 = parse(yaml).unwrap();
        let mut registry2 = Registry::new();
        registry2.register("bg", CMD_BG, YAMAMVA_PASS);
        registry2.register("text", CMD_TEXT, YAMAMVA_PASS);
        registry2.register("speaker", CMD_SPEAKER, YAMAMVA_PASS);
        registry2.register("hearingmenu", CMD_MENU, YAMAMVA_BLOCKING);

        let mut engine2 = restore(scenario2, registry2, &save_json).unwrap();
        let mut args2 = ExecArgs::new();

        // Saved after "Part 2" (node_index 3), so next is end
        assert_eq!(engine2.exec(&mut args2), YAMAMVA_END);

        // State should be restored
        assert_eq!(engine2.state().get_value("heard_elmar"), Some(Value::Bool(true)));
        assert_eq!(engine2.state().get_value("hearing_count"), Some(Value::Int(1)));
    }
}
