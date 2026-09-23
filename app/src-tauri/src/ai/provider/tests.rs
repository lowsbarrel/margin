use super::anthropic::{MAX_TOKENS, MAX_TOKENS_WITH_THINKING};
use super::*;
use crate::ai::chat::{ToolCall, ToolOutcome};
use crate::ai::config::Effort;
use serde_json::json;

fn config(format: ApiFormat, effort: Option<Effort>) -> LlmConfig {
    LlmConfig {
        api_format: format,
        base_url: "https://x.test/v1".into(),
        api_key: "k".into(),
        model: "m".into(),
        effort,
    }
}

#[test]
fn effort_reaches_each_request_shape_only_when_it_is_set() {
    let tools = [ToolSpec {
        name: "search",
        description: "d",
        parameters: json!({"type": "object"}),
    }];
    let conversation = vec![ChatMessage::User("q".into())];

    let plain = config(ApiFormat::Openai, None);
    let openai = openai::request_body(&plain, "sys", &conversation, &tools);
    assert!(openai.get("reasoning_effort").is_none());

    let openai = openai::request_body(
        &config(ApiFormat::Openai, Some(Effort::High)),
        "sys",
        &conversation,
        &tools,
    );
    assert_eq!(openai["reasoning_effort"], "high");

    let anthropic = anthropic::request_body(&plain, "sys", &conversation, &tools);
    assert!(anthropic.get("thinking").is_none());
    assert!(anthropic.get("output_config").is_none());
    assert_eq!(anthropic["max_tokens"], MAX_TOKENS);

    let anthropic = anthropic::request_body(
        &config(ApiFormat::Anthropic, Some(Effort::Medium)),
        "sys",
        &conversation,
        &tools,
    );
    assert_eq!(anthropic["thinking"]["type"], "adaptive");
    assert_eq!(anthropic["output_config"]["effort"], "medium");
    assert_eq!(anthropic["max_tokens"], MAX_TOKENS_WITH_THINKING);
}

#[test]
fn message_shapes_are_translated_per_provider() {
    let conversation = vec![
        ChatMessage::User("what did I decide?".into()),
        ChatMessage::Assistant {
            text: "Checking.".into(),
            calls: vec![ToolCall {
                id: "call_1".into(),
                name: "search".into(),
                args: json!({"query": "decision"}),
            }],
            reasoning: Vec::new(),
        },
        ChatMessage::ToolResults(vec![ToolOutcome {
            call_id: "call_1".into(),
            content: "no hits".into(),
        }]),
    ];

    let openai = openai::wire_messages("sys", &conversation);
    assert_eq!(openai[0]["role"], "system");
    assert!(openai[2].get("reasoning_details").is_none());
    assert_eq!(openai[2]["tool_calls"][0]["function"]["name"], "search");
    assert_eq!(
        openai[2]["tool_calls"][0]["function"]["arguments"],
        "{\"query\":\"decision\"}"
    );
    assert_eq!(openai[3]["role"], "tool");
    assert_eq!(openai[3]["tool_call_id"], "call_1");

    let anthropic = anthropic::wire_messages(&conversation);
    assert_eq!(
        anthropic.len(),
        3,
        "system is a top-level field, not a message"
    );
    assert_eq!(anthropic[1]["content"][1]["type"], "tool_use");
    assert_eq!(anthropic[1]["content"][1]["input"]["query"], "decision");
    assert_eq!(anthropic[2]["role"], "user");
    assert_eq!(anthropic[2]["content"][0]["type"], "tool_result");
    assert_eq!(anthropic[2]["content"][0]["tool_use_id"], "call_1");
}

#[test]
fn provider_error_message_prefers_the_nested_message() {
    assert_eq!(
        provider_error_message("{\"error\":{\"message\":\"bad key\"}}"),
        "bad key"
    );
    assert_eq!(
        provider_error_message("{\"error\":\"overloaded\"}"),
        "overloaded"
    );
    assert_eq!(provider_error_message("  plain text  "), "plain text");
    assert!(!provider_error_message("").is_empty());
}
