use std::collections::BTreeMap;

use margin_ask::ToolCall;
use serde_json::{Map, Value, json};

pub(crate) type ReasoningBlocks = BTreeMap<u64, Map<String, Value>>;

pub(crate) fn accumulate_reasoning(index: u64, fields: &Value, into: &mut ReasoningBlocks) {
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
                    block.insert(key.clone(), Value::String(part.to_owned()));
                }
            },
            _ => {
                block.insert(key.clone(), value.clone());
            }
        }
    }
}

#[derive(Default)]
pub(crate) struct PartialCall {
    pub id: String,
    pub name: String,
    pub args: String,
}

pub(crate) type PartialCalls = BTreeMap<usize, PartialCall>;

pub(crate) fn finish_calls(calls: PartialCalls) -> Vec<ToolCall> {
    calls
        .into_values()
        .map(|call| ToolCall {
            id: call.id,
            name: call.name,
            args: serde_json::from_str(&call.args).unwrap_or_else(|_| json!({})),
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn tool_calls_with_empty_or_invalid_arguments_default_to_an_empty_object() {
        let mut calls = PartialCalls::new();
        calls.insert(
            0,
            PartialCall {
                id: "call_1".to_owned(),
                name: "list_tags".to_owned(),
                args: String::new(),
            },
        );
        calls.insert(
            1,
            PartialCall {
                id: "call_2".to_owned(),
                name: "recent_notes".to_owned(),
                args: "not json".to_owned(),
            },
        );
        let calls = finish_calls(calls);
        assert_eq!(calls[0].args, json!({}));
        assert_eq!(calls[1].args, json!({}));
    }
}
