use std::collections::HashMap;

pub const YAMAMVA_END: i32 = -1;
pub const YAMAMVA_PASS: i32 = 0;
pub const YAMAMVA_BLOCKING: i32 = 1;

#[derive(Debug, Clone)]
pub struct CommandEntry {
    pub command_id: i32,
    pub blocking: bool,
}

#[derive(Debug, Clone)]
pub struct Registry {
    table: HashMap<String, CommandEntry>,
}

impl Registry {
    pub fn new() -> Self {
        Registry {
            table: HashMap::new(),
        }
    }

    pub fn register(&mut self, node_type: &str, command_id: i32, flags: i32) {
        self.table.insert(
            node_type.to_string(),
            CommandEntry {
                command_id,
                blocking: flags == YAMAMVA_BLOCKING,
            },
        );
    }

    pub fn lookup(&self, node_type: &str) -> Option<&CommandEntry> {
        self.table.get(node_type)
    }
}

impl Default for Registry {
    fn default() -> Self {
        Self::new()
    }
}
