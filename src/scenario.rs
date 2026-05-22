use std::collections::HashMap;

#[derive(Debug, Clone)]
pub struct Scenario {
    pub id: String,
    pub title: String,
    pub version: Option<String>,
    pub entry: String,
    pub initial_state: HashMap<String, serde_json::Value>,
    pub meta: HashMap<String, serde_json::Value>,
    pub scenes: HashMap<String, Vec<Node>>,
}

#[derive(Debug, Clone)]
pub struct Node {
    pub node_type: String,
    pub when: Option<String>,
    pub raw: serde_json::Value,
    pub elements: Option<Vec<Element>>,
    pub branches: Option<Vec<Branch>>,
}

#[derive(Debug, Clone)]
pub struct Element {
    pub key: String,
    pub label: Option<String>,
    pub when: Option<String>,
    pub extra: serde_json::Value,
}

#[derive(Debug, Clone)]
pub struct Branch {
    pub when: Option<String>,
    pub do_updates: Option<HashMap<String, String>>,
    pub next: Option<String>,
}
