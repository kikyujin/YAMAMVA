use std::collections::HashMap;
use crate::scenario::{Scenario, Node, Element, Branch};

#[derive(Debug)]
pub struct ParseError {
    pub message: String,
}

impl std::fmt::Display for ParseError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "YamAMVA parse error: {}", self.message)
    }
}

impl std::error::Error for ParseError {}

pub fn parse(yaml_str: &str) -> Result<Scenario, ParseError> {
    let doc: serde_yaml::Value = serde_yaml::from_str(yaml_str)
        .map_err(|e| ParseError { message: format!("YAML syntax error: {}", e) })?;

    let map = doc.as_mapping()
        .ok_or_else(|| ParseError { message: "top-level must be a mapping".into() })?;

    let id = get_string(&doc, "id")
        .ok_or_else(|| ParseError { message: "missing required field: id".into() })?;
    let title = get_string(&doc, "title")
        .ok_or_else(|| ParseError { message: "missing required field: title".into() })?;
    let version = get_string(&doc, "version");
    let entry = get_string(&doc, "entry")
        .ok_or_else(|| ParseError { message: "missing required field: entry".into() })?;

    let initial_state = parse_state(&doc)?;

    let mut meta = HashMap::new();
    for key in &["characters", "backgrounds", "bgm", "format", "audio"] {
        if let Some(val) = map.get(serde_yaml::Value::String(key.to_string())) {
            let json_val = yaml_to_json(val);
            meta.insert(key.to_string(), json_val);
        }
    }

    let scenes = parse_scenes(&doc)?;

    Ok(Scenario {
        id,
        title,
        version,
        entry,
        initial_state,
        meta,
        scenes,
    })
}

fn get_string(doc: &serde_yaml::Value, key: &str) -> Option<String> {
    doc.get(key).and_then(|v| match v {
        serde_yaml::Value::String(s) => Some(s.clone()),
        serde_yaml::Value::Number(n) => Some(n.to_string()),
        serde_yaml::Value::Bool(b) => Some(b.to_string()),
        _ => None,
    })
}

fn parse_state(doc: &serde_yaml::Value) -> Result<HashMap<String, serde_json::Value>, ParseError> {
    let mut result = HashMap::new();
    if let Some(state_val) = doc.get("state")
        && let Some(map) = state_val.as_mapping() {
            for (k, v) in map {
                if let Some(key) = k.as_str() {
                    result.insert(key.to_string(), yaml_to_json(v));
                }
            }
        }
    Ok(result)
}

fn parse_scenes(doc: &serde_yaml::Value) -> Result<HashMap<String, Vec<Node>>, ParseError> {
    let scenes_val = doc.get("scenes")
        .ok_or_else(|| ParseError { message: "missing required field: scenes".into() })?;
    let scenes_map = scenes_val.as_mapping()
        .ok_or_else(|| ParseError { message: "scenes must be a mapping".into() })?;

    let mut result = HashMap::new();
    for (key, val) in scenes_map {
        let scene_id = key.as_str()
            .ok_or_else(|| ParseError { message: "scene key must be a string".into() })?
            .to_string();
        let nodes_seq = val.as_sequence()
            .ok_or_else(|| ParseError { message: format!("scene '{}' must be a list of nodes", scene_id) })?;

        let mut nodes = Vec::new();
        for (idx, node_val) in nodes_seq.iter().enumerate() {
            let node = parse_node(node_val, &scene_id, idx)?;
            nodes.push(node);
        }
        result.insert(scene_id, nodes);
    }
    Ok(result)
}

fn parse_node(val: &serde_yaml::Value, scene_id: &str, idx: usize) -> Result<Node, ParseError> {
    let map = val.as_mapping()
        .ok_or_else(|| ParseError {
            message: format!("node at {}[{}] must be a mapping", scene_id, idx)
        })?;

    let when = val.get("when").and_then(|v| v.as_str()).map(|s| s.to_string());

    let node_type = determine_node_type(map);

    let raw = build_raw_json(map, &node_type);

    let elements = parse_elements(val);
    let branches = parse_branches(val, &node_type);

    Ok(Node {
        node_type,
        when,
        raw,
        elements,
        branches,
    })
}

fn determine_node_type(map: &serde_yaml::Mapping) -> String {
    let priority_keys = ["end", "do", "jump", "incase", "choice"];
    for key in &priority_keys {
        if map.contains_key(serde_yaml::Value::String(key.to_string())) {
            return key.to_string();
        }
    }

    if map.contains_key(serde_yaml::Value::String("speaker".into()))
        && map.contains_key(serde_yaml::Value::String("text".into()))
    {
        return "speaker".to_string();
    }

    if map.contains_key(serde_yaml::Value::String("text".into())) {
        return "text".to_string();
    }

    if map.contains_key(serde_yaml::Value::String("bg".into())) {
        return "bg".to_string();
    }

    if map.contains_key(serde_yaml::Value::String("bgm".into())) {
        return "bgm".to_string();
    }

    let skip_keys = ["when"];
    for (k, _) in map {
        if let Some(key_str) = k.as_str()
            && !skip_keys.contains(&key_str) {
                return key_str.to_string();
            }
    }

    "unknown".to_string()
}

fn build_raw_json(map: &serde_yaml::Mapping, _node_type: &str) -> serde_json::Value {
    let mut json_map = serde_json::Map::new();
    for (k, v) in map {
        if let Some(key) = k.as_str() {
            if key == "when" {
                continue;
            }
            json_map.insert(key.to_string(), yaml_to_json(v));
        }
    }
    serde_json::Value::Object(json_map)
}

fn parse_elements(val: &serde_yaml::Value) -> Option<Vec<Element>> {
    // elements can be nested inside the node's primary value
    // e.g. hearingmenu: { style: ..., elements: [...] }
    // or choice: { options: [...] }

    // First, try direct elements field
    let elements_val = find_elements(val)?;
    let seq = elements_val.as_sequence()?;

    let mut result = Vec::new();
    for item in seq {
        if let Some(map) = item.as_mapping() {
            let key = item.get("key").and_then(|v| v.as_str()).unwrap_or("").to_string();
            let label = item.get("label").and_then(|v| v.as_str()).map(|s| s.to_string())
                .or_else(|| item.get("text").and_then(|v| v.as_str()).map(|s| s.to_string()));
            let when = item.get("when").and_then(|v| v.as_str()).map(|s| s.to_string());

            let mut extra = serde_json::Map::new();
            for (k, v) in map {
                if let Some(ks) = k.as_str()
                    && ks != "key" && ks != "label" && ks != "when" {
                        extra.insert(ks.to_string(), yaml_to_json(v));
                    }
            }

            result.push(Element {
                key,
                label,
                when,
                extra: serde_json::Value::Object(extra),
            });
        }
    }

    if result.is_empty() { None } else { Some(result) }
}

fn find_elements(val: &serde_yaml::Value) -> Option<&serde_yaml::Value> {
    // Direct elements field on node
    if let Some(el) = val.get("elements") {
        return Some(el);
    }

    // Check inside the primary value (e.g., hearingmenu: { elements: [...] })
    if let Some(map) = val.as_mapping() {
        for (k, v) in map {
            if let Some(key) = k.as_str() {
                if key == "when" { continue; }
                if let Some(inner_map) = v.as_mapping()
                    && let Some(el) = inner_map.get(serde_yaml::Value::String("elements".into())) {
                        return Some(el);
                    }
            }
        }
    }

    // choice: { options: [...] } → treat options as elements
    if let Some(choice_val) = val.get("choice")
        && let Some(options) = choice_val.get("options") {
            return Some(options);
        }

    None
}

fn parse_branches(val: &serde_yaml::Value, node_type: &str) -> Option<Vec<Branch>> {
    let branch_val = match node_type {
        "jump" => val.get("jump")?,
        "incase" => val.get("incase")?,
        _ => return None,
    };

    let seq = branch_val.as_sequence()?;
    let mut branches = Vec::new();

    for item in seq {
        let when = item.get("when").and_then(|v| v.as_str()).map(|s| s.to_string());
        let next = item.get("next").and_then(|v| v.as_str()).map(|s| s.to_string());

        let do_updates = if let Some(do_val) = item.get("do") {
            if let Some(map) = do_val.as_mapping() {
                let mut updates = HashMap::new();
                for (k, v) in map {
                    if let Some(key) = k.as_str() {
                        let val_str = match v {
                            serde_yaml::Value::String(s) => s.clone(),
                            serde_yaml::Value::Bool(b) => b.to_string(),
                            serde_yaml::Value::Number(n) => n.to_string(),
                            _ => serde_json::to_string(&yaml_to_json(v)).unwrap_or_default(),
                        };
                        updates.insert(key.to_string(), val_str);
                    }
                }
                Some(updates)
            } else {
                None
            }
        } else {
            None
        };

        branches.push(Branch {
            when,
            do_updates,
            next,
        });
    }

    if branches.is_empty() { None } else { Some(branches) }
}

fn yaml_to_json(val: &serde_yaml::Value) -> serde_json::Value {
    match val {
        serde_yaml::Value::Null => serde_json::Value::Null,
        serde_yaml::Value::Bool(b) => serde_json::Value::Bool(*b),
        serde_yaml::Value::Number(n) => {
            if let Some(i) = n.as_i64() {
                serde_json::json!(i)
            } else if let Some(f) = n.as_f64() {
                serde_json::json!(f)
            } else {
                serde_json::Value::Null
            }
        }
        serde_yaml::Value::String(s) => serde_json::Value::String(s.clone()),
        serde_yaml::Value::Sequence(seq) => {
            serde_json::Value::Array(seq.iter().map(yaml_to_json).collect())
        }
        serde_yaml::Value::Mapping(map) => {
            let mut json_map = serde_json::Map::new();
            for (k, v) in map {
                if let Some(key) = k.as_str() {
                    json_map.insert(key.to_string(), yaml_to_json(v));
                }
            }
            serde_json::Value::Object(json_map)
        }
        serde_yaml::Value::Tagged(tagged) => yaml_to_json(&tagged.value),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_minimal() {
        let yaml = r#"
id: test
title: "Test Scenario"
entry: scene_start
scenes:
  scene_start:
    - text: "Hello"
    - end: true
"#;
        let scenario = parse(yaml).unwrap();
        assert_eq!(scenario.id, "test");
        assert_eq!(scenario.entry, "scene_start");
        let scene = &scenario.scenes["scene_start"];
        assert_eq!(scene.len(), 2);
        assert_eq!(scene[0].node_type, "text");
        assert_eq!(scene[1].node_type, "end");
    }

    #[test]
    fn test_parse_full_scenario() {
        let yaml = include_str!("../examples/oyatsu_adv.yaml");
        let scenario = parse(yaml).unwrap();
        assert_eq!(scenario.id, "oyatsu_adv");
        assert_eq!(scenario.entry, "scene_intro");
        assert!(scenario.scenes.contains_key("scene_intro"));
        assert!(scenario.scenes.contains_key("scene_hearing_menu"));
        assert!(scenario.scenes.contains_key("scene_hear_elmar"));
        assert!(scenario.scenes.contains_key("scene_ending_win"));
    }

    #[test]
    fn test_parse_elements() {
        let yaml = r#"
id: test
title: test
entry: scene_start
scenes:
  scene_start:
    - hearingmenu:
        style: vertical
        elements:
          - { key: elmar, label: "エルマーのラボへ", when: "not heard_elmar" }
          - { key: leave, label: "帰る" }
    - end: true
"#;
        let scenario = parse(yaml).unwrap();
        let scene = &scenario.scenes["scene_start"];
        let menu_node = &scene[0];
        assert_eq!(menu_node.node_type, "hearingmenu");
        let elements = menu_node.elements.as_ref().unwrap();
        assert_eq!(elements.len(), 2);
        assert_eq!(elements[0].key, "elmar");
        assert_eq!(elements[0].when.as_deref(), Some("not heard_elmar"));
        assert_eq!(elements[1].key, "leave");
        assert!(elements[1].when.is_none());
    }

    #[test]
    fn test_parse_jump_branches() {
        let yaml = r#"
id: test
title: test
entry: scene_start
scenes:
  scene_start:
    - jump:
        - when: "score >= 80"
          next: scene_good
        - next: scene_bad
"#;
        let scenario = parse(yaml).unwrap();
        let scene = &scenario.scenes["scene_start"];
        let jump = &scene[0];
        assert_eq!(jump.node_type, "jump");
        let branches = jump.branches.as_ref().unwrap();
        assert_eq!(branches.len(), 2);
        assert_eq!(branches[0].when.as_deref(), Some("score >= 80"));
        assert_eq!(branches[0].next.as_deref(), Some("scene_good"));
        assert!(branches[1].when.is_none());
        assert_eq!(branches[1].next.as_deref(), Some("scene_bad"));
    }

    #[test]
    fn test_parse_incase() {
        let yaml = r#"
id: test
title: test
entry: scene_start
scenes:
  scene_start:
    - incase:
        - when: "$result == 'elmar'"
          do: { heard_elmar: true }
          next: scene_hear_elmar
        - next: scene_ending
"#;
        let scenario = parse(yaml).unwrap();
        let scene = &scenario.scenes["scene_start"];
        let incase = &scene[0];
        assert_eq!(incase.node_type, "incase");
        let branches = incase.branches.as_ref().unwrap();
        assert_eq!(branches.len(), 2);
        assert_eq!(branches[0].when.as_deref(), Some("$result == 'elmar'"));
        let do_updates = branches[0].do_updates.as_ref().unwrap();
        assert_eq!(do_updates.get("heard_elmar").unwrap(), "true");
        assert_eq!(branches[0].next.as_deref(), Some("scene_hear_elmar"));
    }
}
