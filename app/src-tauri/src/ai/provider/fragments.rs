use crate::ai::chat::ToolCall;
use serde_json::{Map, Value, json};
use std::collections::BTreeMap;

pub(super) type ReasoningBlocks = BTreeMap<u64, Map<String, Value>>;

pub(super) fn accumulate_reasoning(index: u64, fields: &Value, into: &mut ReasoningBlocks) {
    let Some(fields) = fields.as_object() else {
        return;
    };
    let block = into.entry(index).or_default();
    for (key, value) in fields {
        if value.is_null() {
            continue;
        }
        let concatenates = matches!(key.as_str(), "text" | "summary" | "data" | "thinking");
        match (concatenates, value.as_str()) {
            (true, Some(part)) => match block.get_mut(key) {
                Some(Value::String(existing)) => existing.push_str(part),
                _ => {
                    block.insert(key.clone(), Value::String(part.to_string()));
                }
            },
            _ => {
                block.insert(key.clone(), value.clone());
            }
        }
    }
}

#[derive(Default)]
pub(super) struct PartialCall {
    pub id: String,
    pub name: String,
    pub args: String,
}

pub(super) type PartialCalls = BTreeMap<usize, PartialCall>;

pub(super) fn finish_calls(calls: PartialCalls) -> Vec<ToolCall> {
    calls
        .into_values()
        .map(|call| ToolCall {
            id: call.id,
            name: call.name,
            args: serde_json::from_str(&call.args).unwrap_or_else(|_| json!({})),
        })
        .collect()
}
