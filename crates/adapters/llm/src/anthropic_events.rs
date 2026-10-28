use std::collections::BTreeMap;

use margin_ask::ToolCall;
use serde_json::{Value, json};

use crate::fragments::{PartialCall, ReasoningBlocks, accumulate_reasoning, finish_calls};

#[derive(Default)]
pub(crate) struct ContentBlock {
    pub id: String,
    pub name: String,
    pub args: String,
}

pub(crate) fn apply_event(
    event: &Value,
    text: &mut String,
    blocks: &mut BTreeMap<usize, ContentBlock>,
    reasoning: &mut ReasoningBlocks,
) {
    let kind = event.get("type").and_then(Value::as_str).unwrap_or("");
    let index = event.get("index").and_then(Value::as_u64).unwrap_or(0);
    let position = usize::try_from(index).unwrap_or(usize::MAX);
    match kind {
        "content_block_start" => {
            let Some(block) = event.get("content_block") else {
                return;
            };
            match block.get("type").and_then(Value::as_str) {
                Some("text") => {
                    if let Some(part) = block.get("text").and_then(Value::as_str) {
                        text.push_str(part);
                    }
                }
                Some("thinking" | "redacted_thinking") => {
                    accumulate_reasoning(index, block, reasoning);
                }
                Some("tool_use") => {
                    let id = block.get("id").and_then(Value::as_str).unwrap_or_default();
                    let name = block
                        .get("name")
                        .and_then(Value::as_str)
                        .unwrap_or_default();
                    let slot = blocks.entry(position).or_default();
                    id.clone_into(&mut slot.id);
                    name.clone_into(&mut slot.name);
                }
                _ => {}
            }
        }
        "content_block_delta" => {
            let Some(delta) = event.get("delta") else {
                return;
            };
            match delta.get("type").and_then(Value::as_str) {
                Some("text_delta") => {
                    if let Some(part) = delta.get("text").and_then(Value::as_str) {
                        text.push_str(part);
                    }
                }
                Some("thinking_delta") => {
                    if let Some(part) = delta.get("thinking") {
                        accumulate_reasoning(index, &json!({ "thinking": part }), reasoning);
                    }
                }
                Some("signature_delta") => {
                    if let Some(signature) = delta.get("signature") {
                        accumulate_reasoning(index, &json!({ "signature": signature }), reasoning);
                    }
                }
                Some("input_json_delta") => {
                    if let Some(part) = delta.get("partial_json").and_then(Value::as_str) {
                        blocks.entry(position).or_default().args.push_str(part);
                    }
                }
                _ => {}
            }
        }
        _ => {}
    }
}

pub(crate) fn tool_calls(blocks: BTreeMap<usize, ContentBlock>) -> Vec<ToolCall> {
    let calls = blocks
        .into_iter()
        .filter(|(_, block)| !block.name.is_empty())
        .map(|(index, block)| {
            (
                index,
                PartialCall {
                    id: block.id,
                    name: block.name,
                    args: block.args,
                },
            )
        })
        .collect();
    finish_calls(calls)
}

#[cfg(test)]
mod tests {
    use margin_ask::{ChatMessage, ToolOutcome};

    use super::*;
    use crate::anthropic::wire_messages;

    #[test]
    fn anthropic_events_build_text_and_tool_use_blocks() {
        let mut text = String::new();
        let mut blocks = BTreeMap::new();
        let mut reasoning = ReasoningBlocks::new();

        apply_event(
            &json!({"type": "content_block_start", "index": 0, "content_block": {"type": "text", "text": ""}}),
            &mut text,
            &mut blocks,
            &mut reasoning,
        );
        apply_event(
            &json!({"type": "content_block_delta", "index": 0, "delta": {"type": "text_delta", "text": "Let me check"}}),
            &mut text,
            &mut blocks,
            &mut reasoning,
        );
        apply_event(
            &json!({"type": "content_block_start", "index": 1, "content_block": {"type": "tool_use", "id": "toolu_1", "name": "read_note"}}),
            &mut text,
            &mut blocks,
            &mut reasoning,
        );
        apply_event(
            &json!({"type": "content_block_delta", "index": 1, "delta": {"type": "input_json_delta", "partial_json": "{\"path\":"}}),
            &mut text,
            &mut blocks,
            &mut reasoning,
        );
        apply_event(
            &json!({"type": "content_block_delta", "index": 1, "delta": {"type": "input_json_delta", "partial_json": "\"notes/a.md\"}"}}),
            &mut text,
            &mut blocks,
            &mut reasoning,
        );
        apply_event(
            &json!({"type": "message_delta", "delta": {"stop_reason": "tool_use"}}),
            &mut text,
            &mut blocks,
            &mut reasoning,
        );

        assert_eq!(text, "Let me check");
        assert_eq!(blocks.len(), 1);
        assert_eq!(blocks[&1].name, "read_note");
        assert_eq!(blocks[&1].args, "{\"path\":\"notes/a.md\"}");
        assert!(reasoning.is_empty());
    }

    #[test]
    fn anthropic_thinking_replays_before_the_tool_use_it_produced() {
        let mut text = String::new();
        let mut blocks = BTreeMap::new();
        let mut reasoning = ReasoningBlocks::new();

        let events = [
            json!({"type": "content_block_start", "index": 0, "content_block": {"type": "thinking", "thinking": ""}}),
            json!({"type": "content_block_delta", "index": 0, "delta": {"type": "thinking_delta", "thinking": "Check the roadmap "}}),
            json!({"type": "content_block_delta", "index": 0, "delta": {"type": "thinking_delta", "thinking": "note."}}),
            json!({"type": "content_block_delta", "index": 0, "delta": {"type": "signature_delta", "signature": "ErUBCkYI"}}),
            json!({"type": "content_block_start", "index": 1, "content_block": {"type": "text", "text": ""}}),
            json!({"type": "content_block_delta", "index": 1, "delta": {"type": "text_delta", "text": "Let me look."}}),
            json!({"type": "content_block_start", "index": 2, "content_block": {"type": "tool_use", "id": "toolu_1", "name": "search"}}),
            json!({"type": "content_block_delta", "index": 2, "delta": {"type": "input_json_delta", "partial_json": "{\"query\":\"roadmap\"}"}}),
        ];
        for event in &events {
            apply_event(event, &mut text, &mut blocks, &mut reasoning);
        }

        assert_eq!(text, "Let me look.", "thinking is not answer text");
        let reasoning: Vec<Value> = reasoning.into_values().map(Value::Object).collect();
        assert_eq!(
            reasoning,
            vec![json!({
                "type": "thinking",
                "thinking": "Check the roadmap note.",
                "signature": "ErUBCkYI",
            })]
        );

        let conversation = vec![
            ChatMessage::User("what did I decide?".to_owned()),
            ChatMessage::Assistant {
                text,
                calls: tool_calls(blocks),
                reasoning,
            },
            ChatMessage::ToolResults(vec![ToolOutcome {
                call_id: "toolu_1".to_owned(),
                content: "no hits".to_owned(),
            }]),
        ];

        let anthropic = wire_messages(&conversation);
        assert_eq!(anthropic[1]["content"][0]["type"], "thinking");
        assert_eq!(
            anthropic[1]["content"][0]["thinking"],
            "Check the roadmap note."
        );
        assert_eq!(anthropic[1]["content"][0]["signature"], "ErUBCkYI");
        assert_eq!(anthropic[1]["content"][1]["type"], "text");
        assert_eq!(anthropic[1]["content"][2]["type"], "tool_use");
        assert_eq!(anthropic[1]["content"][2]["name"], "search");
        assert_eq!(anthropic[2]["content"][0]["type"], "tool_result");
    }

    #[test]
    fn anthropic_redacted_thinking_is_stored_and_replayed_verbatim() {
        let mut text = String::new();
        let mut blocks = BTreeMap::new();
        let mut reasoning = ReasoningBlocks::new();

        apply_event(
            &json!({"type": "content_block_start", "index": 0, "content_block": {"type": "redacted_thinking", "data": "EmwKAhgB"}}),
            &mut text,
            &mut blocks,
            &mut reasoning,
        );

        let stored: Vec<Value> = reasoning.into_values().map(Value::Object).collect();
        assert_eq!(
            stored,
            vec![json!({"type": "redacted_thinking", "data": "EmwKAhgB"})]
        );

        let conversation = vec![ChatMessage::Assistant {
            text: "Answering.".to_owned(),
            calls: Vec::new(),
            reasoning: stored,
        }];
        let anthropic = wire_messages(&conversation);
        assert_eq!(anthropic[0]["content"][0]["type"], "redacted_thinking");
        assert_eq!(anthropic[0]["content"][0]["data"], "EmwKAhgB");
        assert_eq!(anthropic[0]["content"][1]["type"], "text");
    }
}
