use crate::evaluator::evaluate_when;
use crate::scenario::{Element, Node};
use crate::state::State;

#[derive(Debug, Clone)]
pub struct FilteredElement {
    pub key: String,
    pub label: Option<String>,
    pub extra_json: String,
}

#[derive(Debug, Clone)]
pub struct BuiltArgs {
    pub node_type: String,
    pub node_json: String,
    pub elements: Vec<FilteredElement>,
}

pub struct ArgsBuilder;

impl ArgsBuilder {
    pub fn build(node: &Node, state: &State) -> BuiltArgs {
        let node_json = serde_json::to_string(&node.raw).unwrap_or_else(|_| "{}".to_string());
        let elements = filter_elements(&node.elements, state);
        BuiltArgs {
            node_type: node.node_type.clone(),
            node_json,
            elements,
        }
    }
}

fn filter_elements(elements: &Option<Vec<Element>>, state: &State) -> Vec<FilteredElement> {
    let Some(elements) = elements else {
        return Vec::new();
    };

    elements
        .iter()
        .filter(|el| {
            el.when.as_ref().is_none_or(|w| evaluate_when(w, state))
        })
        .map(|el| FilteredElement {
            key: el.key.clone(),
            label: el.label.clone(),
            extra_json: serde_json::to_string(&el.extra).unwrap_or_else(|_| "{}".to_string()),
        })
        .collect()
}
