use super::*;
use crate::ai::chat::ToolOutcome;

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
        ChatMessage::User("what did I decide?".into()),
        ChatMessage::Assistant {
            text,
            calls: tool_calls(blocks),
            reasoning,
        },
        ChatMessage::ToolResults(vec![ToolOutcome {
            call_id: "toolu_1".into(),
            content: "no hits".into(),
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
        text: "Answering.".into(),
        calls: Vec::new(),
        reasoning: stored,
    }];
    let anthropic = wire_messages(&conversation);
    assert_eq!(anthropic[0]["content"][0]["type"], "redacted_thinking");
    assert_eq!(anthropic[0]["content"][0]["data"], "EmwKAhgB");
    assert_eq!(anthropic[0]["content"][1]["type"], "text");
}
