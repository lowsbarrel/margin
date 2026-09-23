use super::*;
use crate::ai::chat::ToolOutcome;

#[test]
fn openai_deltas_accumulate_a_tool_call_split_over_chunks() {
    let mut text = String::new();
    let mut calls = PartialCalls::new();
    let mut reasoning = ReasoningBlocks::new();

    apply_delta(
        &json!({"content": "Looking", "tool_calls": [{"index": 0, "id": "call_1", "type": "function", "function": {"name": "search", "arguments": "{\"quer"}}]}),
        &mut text,
        &mut calls,
        &mut reasoning,
    );
    apply_delta(
        &json!({"content": " that up", "tool_calls": [{"index": 0, "function": {"arguments": "y\":\"roadmap\"}"}}]}),
        &mut text,
        &mut calls,
        &mut reasoning,
    );

    assert_eq!(text, "Looking that up");
    let finished = finish_calls(calls);
    assert_eq!(finished.len(), 1);
    assert_eq!(finished[0].name, "search");
    assert_eq!(finished[0].id, "call_1");
    assert_eq!(finished[0].args, json!({"query": "roadmap"}));
}

#[test]
fn openai_keeps_parallel_tool_calls_apart_by_index() {
    let mut text = String::new();
    let mut calls = PartialCalls::new();
    let mut reasoning = ReasoningBlocks::new();
    apply_delta(
        &json!({"tool_calls": [
            {"index": 0, "id": "a", "function": {"name": "search", "arguments": "{}"}},
            {"index": 1, "id": "b", "function": {"name": "list_tags", "arguments": "{}"}}
        ]}),
        &mut text,
        &mut calls,
        &mut reasoning,
    );
    let names: Vec<String> = finish_calls(calls).into_iter().map(|c| c.name).collect();
    assert_eq!(names, vec!["search".to_string(), "list_tags".to_string()]);
}

#[test]
fn unparseable_tool_arguments_become_null_rather_than_panicking() {
    let mut text = String::new();
    let mut calls = PartialCalls::new();
    let mut reasoning = ReasoningBlocks::new();
    apply_delta(
        &json!({"tool_calls": [{"index": 0, "id": "a", "function": {"name": "grep", "arguments": "{\"pattern\": "}}]}),
        &mut text,
        &mut calls,
        &mut reasoning,
    );
    let finished = finish_calls(calls);
    assert!(finished[0].args.is_null());
}

#[test]
fn openai_reasoning_details_accumulate_and_ride_back_with_the_tool_results() {
    let mut text = String::new();
    let mut calls = PartialCalls::new();
    let mut reasoning = ReasoningBlocks::new();

    apply_delta(
        &json!({"reasoning_details": [
            {"type": "reasoning.text", "index": 0, "text": "The user ", "format": "anthropic-claude-v1"}
        ]}),
        &mut text,
        &mut calls,
        &mut reasoning,
    );
    apply_delta(
        &json!({"reasoning_details": [{"index": 0, "text": "asked about ", "signature": "sig-1"}]}),
        &mut text,
        &mut calls,
        &mut reasoning,
    );
    apply_delta(
        &json!({"reasoning_details": [
            {"index": 0, "text": "the roadmap.", "signature": null, "id": null}
        ]}),
        &mut text,
        &mut calls,
        &mut reasoning,
    );
    apply_delta(
        &json!({
            "reasoning_details": [{"type": "reasoning.summary", "index": 1, "summary": "short"}],
            "tool_calls": [{"index": 0, "id": "call_1", "function": {"name": "search", "arguments": "{}"}}],
        }),
        &mut text,
        &mut calls,
        &mut reasoning,
    );

    assert_eq!(text, "", "reasoning is never answer text");
    let turn = AssistantTurn {
        text,
        calls: finish_calls(calls),
        reasoning: reasoning.into_values().map(Value::Object).collect(),
    };
    assert_eq!(turn.reasoning.len(), 2);
    assert_eq!(turn.reasoning[0]["type"], "reasoning.text");
    assert_eq!(
        turn.reasoning[0]["text"],
        "The user asked about the roadmap."
    );
    assert_eq!(turn.reasoning[0]["signature"], "sig-1");
    assert_eq!(turn.reasoning[0]["format"], "anthropic-claude-v1");
    assert_eq!(turn.reasoning[1]["type"], "reasoning.summary");
    assert_eq!(turn.reasoning[1]["summary"], "short");

    let conversation = vec![
        ChatMessage::User("what did I decide?".into()),
        ChatMessage::Assistant {
            text: turn.text,
            calls: turn.calls,
            reasoning: turn.reasoning,
        },
        ChatMessage::ToolResults(vec![ToolOutcome {
            call_id: "call_1".into(),
            content: "no hits".into(),
        }]),
    ];
    let openai = wire_messages("sys", &conversation);
    assert_eq!(openai[2]["role"], "assistant");
    assert_eq!(
        openai[2]["reasoning_details"][0]["text"],
        "The user asked about the roadmap."
    );
    assert_eq!(openai[2]["reasoning_details"][0]["signature"], "sig-1");
    assert_eq!(
        openai[2]["reasoning_details"][1]["type"],
        "reasoning.summary"
    );
    assert_eq!(openai[3]["role"], "tool");
}
