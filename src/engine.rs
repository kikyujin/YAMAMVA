use std::collections::HashMap;
use crate::args::{ArgsBuilder, BuiltArgs};
use crate::error::YamamvaError;
use crate::evaluator::{evaluate_when, evaluate_do_value};
use crate::parser::parse_file_scene_ref;
use crate::registry::{Registry, YAMAMVA_END};
use crate::scenario::{Node, Scenario};
use crate::state::State;

#[derive(Debug, Clone)]
pub struct Cursor {
    pub scene_id: String,
    pub node_index: usize,
}

pub struct ExecArgs {
    pub result: Option<String>,
    pub built: Option<BuiltArgs>,
}

impl ExecArgs {
    pub fn new() -> Self {
        ExecArgs {
            result: None,
            built: None,
        }
    }
}

impl Default for ExecArgs {
    fn default() -> Self {
        Self::new()
    }
}

pub struct Engine {
    pub(crate) scenario: Scenario,
    pub(crate) state: State,
    pub(crate) registry: Registry,
    pub(crate) cursor: Cursor,
    pub(crate) current_file: String,
    pub(crate) waiting_result: bool,
    pub(crate) warnings: Vec<String>,
}

impl Engine {
    pub fn new(scenario: Scenario, registry: Registry) -> Self {
        let state = State::from_json_map(&scenario.initial_state);
        let entry_scene = match parse_file_scene_ref(&scenario.entry) {
            Ok((_, scene)) => scene,
            Err(_) => scenario.entry.clone(),
        };
        let current_file = scenario.scene_file
            .get(&entry_scene)
            .cloned()
            .unwrap_or_else(|| "__root__".to_string());
        Engine {
            scenario,
            state,
            registry,
            cursor: Cursor {
                scene_id: entry_scene,
                node_index: 0,
            },
            current_file,
            waiting_result: false,
            warnings: Vec::new(),
        }
    }

    pub fn exec(&mut self, args: &mut ExecArgs) -> i32 {
        if self.waiting_result {
            self.state.set_result(args.result.take());
            self.waiting_result = false;
            self.cursor.node_index += 1;
        }

        loop {
            let node = match self.current_node() {
                Some(n) => n.clone(),
                None => return YAMAMVA_END,
            };

            if let Some(ref when_expr) = node.when
                && !evaluate_when(when_expr, &self.state) {
                    self.cursor.node_index += 1;
                    continue;
                }

            match node.node_type.as_str() {
                "end" => {
                    return YAMAMVA_END;
                }

                "do" => {
                    self.apply_do(&node);
                    self.cursor.node_index += 1;
                    continue;
                }

                "jump" => {
                    if let Some(ref branches) = node.branches {
                        if let Some(target) = self.evaluate_branches(branches) {
                            self.apply_branch_do(&target);
                            if let Some(ref next) = target.next {
                                match self.resolve_next(next) {
                                    Ok(scene_id) => {
                                        self.cursor.scene_id = scene_id;
                                        self.cursor.node_index = 0;
                                    }
                                    Err(e) => {
                                        self.warnings.push(e.to_string());
                                        self.cursor.node_index += 1;
                                    }
                                }
                            } else {
                                self.cursor.node_index += 1;
                            }
                        } else {
                            self.cursor.node_index += 1;
                        }
                    } else {
                        self.cursor.node_index += 1;
                    }
                    continue;
                }

                "incase" => {
                    if let Some(ref branches) = node.branches {
                        if let Some(target) = self.evaluate_branches(branches) {
                            self.apply_branch_do(&target);
                            if let Some(ref next) = target.next {
                                match self.resolve_next(next) {
                                    Ok(scene_id) => {
                                        self.cursor.scene_id = scene_id;
                                        self.cursor.node_index = 0;
                                    }
                                    Err(e) => {
                                        self.warnings.push(e.to_string());
                                        self.cursor.node_index += 1;
                                    }
                                }
                            } else {
                                self.cursor.node_index += 1;
                            }
                        } else {
                            self.cursor.node_index += 1;
                        }
                    } else {
                        self.cursor.node_index += 1;
                    }
                    self.state.set_result(None);
                    continue;
                }

                _ => {
                    if let Some(entry) = self.registry.lookup(&node.node_type) {
                        let built = ArgsBuilder::build(&node, &self.state);
                        let command_id = entry.command_id;
                        let blocking = entry.blocking;

                        args.built = Some(built);

                        if blocking {
                            self.waiting_result = true;
                        } else {
                            self.cursor.node_index += 1;
                        }

                        return command_id;
                    } else {
                        self.warnings.push(format!("unregistered: {}", node.node_type));
                        self.cursor.node_index += 1;
                        continue;
                    }
                }
            }
        }
    }

    fn current_node(&self) -> Option<&Node> {
        self.scenario
            .scenes
            .get(&self.cursor.scene_id)?
            .get(self.cursor.node_index)
    }

    fn apply_do(&mut self, node: &Node) {
        if let serde_json::Value::Object(map) = &node.raw
            && let Some(serde_json::Value::Object(do_map)) = map.get("do") {
                for (k, v) in do_map {
                    let val = evaluate_do_value(k, v, &self.state);
                    self.state.set(k, val);
                }
            }
    }

    fn evaluate_branches(&self, branches: &[crate::scenario::Branch]) -> Option<BranchTarget> {
        for branch in branches {
            let condition_met = match &branch.when {
                Some(expr) => evaluate_when(expr, &self.state),
                None => true,
            };
            if condition_met {
                return Some(BranchTarget {
                    do_updates: branch.do_updates.clone(),
                    next: branch.next.clone(),
                });
            }
        }
        None
    }

    fn apply_branch_do(&mut self, target: &BranchTarget) {
        if let Some(ref updates) = target.do_updates {
            for (k, v) in updates {
                let json_val = parse_branch_do_value(v);
                let val = evaluate_do_value(k, &json_val, &self.state);
                self.state.set(k, val);
            }
        }
    }

    fn resolve_next(&mut self, next_ref: &str) -> Result<String, YamamvaError> {
        let (file_opt, scene_id) = parse_file_scene_ref(next_ref)?;

        match file_opt {
            Some(file_stem) => {
                if !self.scenario.scenes.contains_key(&scene_id) {
                    return Err(YamamvaError::Runtime(format!(
                        "scene '{}' not found (referenced as '{}:{}')",
                        scene_id, file_stem, scene_id
                    )));
                }
                self.current_file = file_stem;
                Ok(scene_id)
            }
            None => {
                if let Some(target_file) = self.scenario.scene_file.get(&scene_id) {
                    if *target_file == self.current_file {
                        Ok(scene_id)
                    } else {
                        Err(YamamvaError::Runtime(format!(
                            "scene '{}' exists in file '{}' but current file is '{}'. \
                             Use '{}:{}' for cross-file reference.",
                            scene_id, target_file, self.current_file,
                            target_file, scene_id
                        )))
                    }
                } else {
                    Err(YamamvaError::Runtime(format!(
                        "scene '{}' not found in current file '{}'",
                        scene_id, self.current_file
                    )))
                }
            }
        }
    }

    pub fn state(&self) -> &State {
        &self.state
    }

    pub fn state_mut(&mut self) -> &mut State {
        &mut self.state
    }

    pub fn scenario(&self) -> &Scenario {
        &self.scenario
    }

    pub fn cursor(&self) -> &Cursor {
        &self.cursor
    }

    pub fn current_file(&self) -> &str {
        &self.current_file
    }

    pub fn warnings(&self) -> &[String] {
        &self.warnings
    }

    pub fn clear_warnings(&mut self) {
        self.warnings.clear();
    }

    pub fn is_waiting_result(&self) -> bool {
        self.waiting_result
    }

    pub fn registry(&self) -> &Registry {
        &self.registry
    }

    pub fn registry_mut(&mut self) -> &mut Registry {
        &mut self.registry
    }
}

struct BranchTarget {
    do_updates: Option<HashMap<String, String>>,
    next: Option<String>,
}

fn parse_branch_do_value(s: &str) -> serde_json::Value {
    if s == "true" {
        return serde_json::Value::Bool(true);
    }
    if s == "false" {
        return serde_json::Value::Bool(false);
    }
    if let Ok(n) = s.parse::<i64>() {
        return serde_json::json!(n);
    }
    if let Ok(f) = s.parse::<f64>() {
        return serde_json::json!(f);
    }
    serde_json::Value::String(s.to_string())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::parser::parse;
    use crate::registry::{YAMAMVA_PASS, YAMAMVA_BLOCKING};

    const CMD_BG: i32 = 1;
    const CMD_TEXT: i32 = 2;
    const CMD_SPEAKER: i32 = 3;
    const CMD_MENU: i32 = 4;
    const CMD_MXBS: i32 = 5;
    const CMD_MINIGAME: i32 = 6;
    const CMD_LLM_CHAT: i32 = 7;

    fn make_engine(yaml: &str) -> Engine {
        let scenario = parse(yaml).unwrap();
        let mut registry = Registry::new();
        registry.register("bg", CMD_BG, YAMAMVA_PASS);
        registry.register("text", CMD_TEXT, YAMAMVA_PASS);
        registry.register("speaker", CMD_SPEAKER, YAMAMVA_PASS);
        registry.register("hearingmenu", CMD_MENU, YAMAMVA_BLOCKING);
        registry.register("mxbs_push", CMD_MXBS, YAMAMVA_PASS);
        registry.register("minigame", CMD_MINIGAME, YAMAMVA_BLOCKING);
        registry.register("llm_chat", CMD_LLM_CHAT, YAMAMVA_BLOCKING);
        Engine::new(scenario, registry)
    }

    #[test]
    fn test_linear_flow() {
        let yaml = r#"
id: test
title: test
entry: scene_start
scenes:
  scene_start:
    - bg: lobby
    - text: "Hello"
    - speaker: elmar
      text: "Hi!"
    - end: true
"#;
        let mut engine = make_engine(yaml);
        let mut args = ExecArgs::new();

        assert_eq!(engine.exec(&mut args), CMD_BG);
        assert_eq!(args.built.as_ref().unwrap().node_type, "bg");

        assert_eq!(engine.exec(&mut args), CMD_TEXT);
        assert_eq!(args.built.as_ref().unwrap().node_type, "text");

        assert_eq!(engine.exec(&mut args), CMD_SPEAKER);
        assert_eq!(args.built.as_ref().unwrap().node_type, "speaker");

        assert_eq!(engine.exec(&mut args), YAMAMVA_END);
    }

    #[test]
    fn test_do_and_jump() {
        let yaml = r#"
id: test
title: test
entry: scene_a
state:
  score: 0
scenes:
  scene_a:
    - text: "Start"
    - do:
        score: 100
    - jump:
        - when: "score >= 80"
          next: scene_good
        - next: scene_bad
  scene_good:
    - text: "Good!"
    - end: true
  scene_bad:
    - text: "Bad"
    - end: true
"#;
        let mut engine = make_engine(yaml);
        let mut args = ExecArgs::new();

        assert_eq!(engine.exec(&mut args), CMD_TEXT);
        assert_eq!(engine.exec(&mut args), CMD_TEXT);
        let built = args.built.as_ref().unwrap();
        assert!(built.node_json.contains("Good!"));
        assert_eq!(engine.exec(&mut args), YAMAMVA_END);
    }

    #[test]
    fn test_blocking_and_incase() {
        let yaml = r#"
id: test
title: test
entry: scene_start
state:
  heard_elmar: false
scenes:
  scene_start:
    - hearingmenu:
        style: vertical
        elements:
          - { key: elmar, label: "Go to Elmar" }
          - { key: leave, label: "Leave" }
    - incase:
        - when: "$result == 'elmar'"
          do: { heard_elmar: true }
          next: scene_elmar
        - next: scene_end
  scene_elmar:
    - text: "Elmar's lab"
    - end: true
  scene_end:
    - text: "Goodbye"
    - end: true
"#;
        let mut engine = make_engine(yaml);
        let mut args = ExecArgs::new();

        let id = engine.exec(&mut args);
        assert_eq!(id, CMD_MENU);
        assert!(engine.is_waiting_result());

        let elements = &args.built.as_ref().unwrap().elements;
        assert_eq!(elements.len(), 2);

        args.result = Some("elmar".to_string());
        let id = engine.exec(&mut args);
        assert_eq!(id, CMD_TEXT);
        let built = args.built.as_ref().unwrap();
        assert!(built.node_json.contains("Elmar's lab"));

        assert_eq!(engine.state().get_value("heard_elmar"), Some(crate::state::Value::Bool(true)));
    }

    #[test]
    fn test_when_filter_elements() {
        let yaml = r#"
id: test
title: test
entry: scene_start
state:
  heard_elmar: true
  hearing_count: 1
scenes:
  scene_start:
    - hearingmenu:
        style: vertical
        elements:
          - { key: elmar, label: "Go to Elmar", when: "not heard_elmar" }
          - { key: accuse, label: "Accuse", when: "hearing_count >= 1" }
          - { key: leave, label: "Leave" }
    - end: true
"#;
        let mut engine = make_engine(yaml);
        let mut args = ExecArgs::new();

        engine.exec(&mut args);
        let elements = &args.built.as_ref().unwrap().elements;
        assert_eq!(elements.len(), 2);
        assert_eq!(elements[0].key, "accuse");
        assert_eq!(elements[1].key, "leave");
    }

    #[test]
    fn test_when_skip_node() {
        let yaml = r#"
id: test
title: test
entry: scene_start
state:
  heard_elmar: false
scenes:
  scene_start:
    - speaker: sumire
      text: "You heard Elmar"
      when: "heard_elmar"
    - text: "Next"
    - end: true
"#;
        let mut engine = make_engine(yaml);
        let mut args = ExecArgs::new();

        let id = engine.exec(&mut args);
        assert_eq!(id, CMD_TEXT);
        assert!(args.built.as_ref().unwrap().node_json.contains("Next"));
    }

    #[test]
    fn test_unregistered_skip() {
        let yaml = r#"
id: test
title: test
entry: scene_start
scenes:
  scene_start:
    - bg: lobby
    - unknown_node:
        foo: bar
    - text: "After unknown"
    - end: true
"#;
        let mut engine = make_engine(yaml);
        let mut args = ExecArgs::new();

        assert_eq!(engine.exec(&mut args), CMD_BG);
        assert_eq!(engine.exec(&mut args), CMD_TEXT);
        assert!(args.built.as_ref().unwrap().node_json.contains("After unknown"));
        assert!(engine.warnings().iter().any(|w| w.contains("unknown_node")));
    }

    #[test]
    fn test_hub_and_spoke() {
        let yaml = r#"
id: test
title: test
entry: scene_menu
state:
  heard_elmar: false
  hearing_count: 0
scenes:
  scene_menu:
    - bg: lobby
    - text: "Choose"
    - hearingmenu:
        style: vertical
        elements:
          - { key: elmar, label: "Elmar", when: "not heard_elmar" }
          - { key: accuse, label: "Accuse", when: "hearing_count >= 1" }
          - { key: leave, label: "Leave" }
    - incase:
        - when: "$result == 'elmar'"
          next: scene_hear_elmar
        - when: "$result == 'accuse'"
          next: scene_accuse
        - next: scene_ending
  scene_hear_elmar:
    - bg: lab
    - speaker: elmar
      text: "Hi!"
    - do:
        heard_elmar: true
        hearing_count: "hearing_count + 1"
    - jump:
        - next: scene_menu
  scene_accuse:
    - text: "Accuse!"
    - end: true
  scene_ending:
    - text: "Bye"
    - end: true
"#;
        let mut engine = make_engine(yaml);
        let mut args = ExecArgs::new();

        assert_eq!(engine.exec(&mut args), CMD_BG);
        assert_eq!(engine.exec(&mut args), CMD_TEXT);

        let id = engine.exec(&mut args);
        assert_eq!(id, CMD_MENU);
        let elements = &args.built.as_ref().unwrap().elements;
        assert_eq!(elements.len(), 2);
        assert_eq!(elements[0].key, "elmar");
        assert_eq!(elements[1].key, "leave");

        args.result = Some("elmar".to_string());

        assert_eq!(engine.exec(&mut args), CMD_BG);
        assert_eq!(engine.exec(&mut args), CMD_SPEAKER);

        assert_eq!(engine.exec(&mut args), CMD_BG);
        assert_eq!(engine.exec(&mut args), CMD_TEXT);

        let id = engine.exec(&mut args);
        assert_eq!(id, CMD_MENU);
        let elements = &args.built.as_ref().unwrap().elements;
        assert_eq!(elements.len(), 2);
        assert_eq!(elements[0].key, "accuse");
        assert_eq!(elements[1].key, "leave");

        args.result = Some("accuse".to_string());
        assert_eq!(engine.exec(&mut args), CMD_TEXT);
        assert_eq!(engine.exec(&mut args), YAMAMVA_END);
    }
}
