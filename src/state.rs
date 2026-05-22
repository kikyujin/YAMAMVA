use std::collections::HashMap;

#[derive(Debug, Clone, PartialEq)]
pub enum Value {
    Bool(bool),
    Int(i64),
    Float(f64),
    Str(String),
}

impl Value {
    pub fn is_truthy(&self) -> bool {
        match self {
            Value::Bool(b) => *b,
            Value::Int(n) => *n != 0,
            Value::Float(f) => *f != 0.0,
            Value::Str(s) => !s.is_empty(),
        }
    }

    pub fn to_json(&self) -> serde_json::Value {
        match self {
            Value::Bool(b) => serde_json::Value::Bool(*b),
            Value::Int(n) => serde_json::json!(*n),
            Value::Float(f) => serde_json::json!(*f),
            Value::Str(s) => serde_json::Value::String(s.clone()),
        }
    }

    pub fn from_json(v: &serde_json::Value) -> Option<Value> {
        match v {
            serde_json::Value::Bool(b) => Some(Value::Bool(*b)),
            serde_json::Value::Number(n) => {
                if let Some(i) = n.as_i64() {
                    Some(Value::Int(i))
                } else { n.as_f64().map(Value::Float) }
            }
            serde_json::Value::String(s) => Some(Value::Str(s.clone())),
            _ => None,
        }
    }
}

#[derive(Debug, Clone)]
pub struct State {
    vars: HashMap<String, Value>,
    result: Option<String>,
}

impl State {
    pub fn new(initial: HashMap<String, Value>) -> Self {
        State {
            vars: initial,
            result: None,
        }
    }

    pub fn from_json_map(map: &HashMap<String, serde_json::Value>) -> Self {
        let mut vars = HashMap::new();
        for (k, v) in map {
            if let Some(val) = Value::from_json(v) {
                vars.insert(k.clone(), val);
            }
        }
        State {
            vars,
            result: None,
        }
    }

    pub fn get(&self, key: &str) -> Option<&Value> {
        if key == "$result" {
            return self.result.as_ref().map(|_| {
                // $result is always a string
                // We can't return a reference to a temporary, so we handle this differently
                unreachable!("use get_result() for $result")
            });
        }
        self.vars.get(key)
    }

    pub fn get_value(&self, key: &str) -> Option<Value> {
        if key == "$result" {
            return self.result.as_ref().map(|s| Value::Str(s.clone()));
        }
        self.vars.get(key).cloned()
    }

    pub fn has_var(&self, key: &str) -> bool {
        if key == "$result" {
            return self.result.is_some();
        }
        self.vars.contains_key(key)
    }

    pub fn set(&mut self, key: &str, value: Value) {
        self.vars.insert(key.to_string(), value);
    }

    pub fn get_result(&self) -> Option<&String> {
        self.result.as_ref()
    }

    pub fn set_result(&mut self, result: Option<String>) {
        self.result = result;
    }

    pub fn dump(&self) -> serde_json::Value {
        let mut map = serde_json::Map::new();
        for (k, v) in &self.vars {
            map.insert(k.clone(), v.to_json());
        }
        serde_json::Value::Object(map)
    }

    pub fn restore(&mut self, json: &serde_json::Value) {
        if let serde_json::Value::Object(map) = json {
            for (k, v) in map {
                if let Some(val) = Value::from_json(v) {
                    self.vars.insert(k.clone(), val);
                }
            }
        }
    }

    pub fn vars(&self) -> &HashMap<String, Value> {
        &self.vars
    }
}
