use std::sync::Arc;

use margin_ask::{ApiFormat, AskEvent, AskService, ChatMessage, Error, LlmConfig, ToolCall};
use margin_kernel::UnixMillis;
use margin_test_support::{
    FixedClock, MemoryVaultReader, ScriptedLanguageModel, ScriptedTurn, block_on,
};
use serde_json::{Value, json};

const NOW: UnixMillis = UnixMillis::from_seconds(1_000_000);

fn config() -> LlmConfig {
    LlmConfig {
        api_format: ApiFormat::Openai,
        base_url: "https://x.test/v1".to_owned(),
        api_key: "k".to_owned(),
        model: "m".to_owned(),
        effort: None,
    }
}

fn call(id: &str, name: &str, args: Value) -> ToolCall {
    ToolCall {
        id: id.to_owned(),
        name: name.to_owned(),
        args,
    }
}

struct Fixture {
    service: AskService,
    model: ScriptedLanguageModel,
}

fn fixture(turns: Vec<ScriptedTurn>, vault: MemoryVaultReader) -> Fixture {
    let model = ScriptedLanguageModel::new(turns);
    let service = AskService::new(
        Arc::new(model.clone()),
        Arc::new(vault),
        Arc::new(FixedClock::new(NOW)),
    );
    service
        .configure(config())
        .expect("the test config is valid");
    Fixture { service, model }
}

fn vault() -> MemoryVaultReader {
    let vault = MemoryVaultReader::new();
    vault.insert(
        "projects/roadmap.md",
        "#roadmap\nShip the **editor** rewrite.\nDeferred: sync conflicts.\n",
    );
    vault.insert("projects/beta.md", "Links to [[roadmap]].\n");
    vault
}

fn ask(fixture: &Fixture, question: &str) -> Vec<AskEvent> {
    let mut events = Vec::new();
    let result = block_on(
        fixture
            .service
            .ask("req-1", question, &mut |event| events.push(event)),
    );
    result.expect("a failed answer arrives as an event, not as an error");
    events
}

fn tool_result(fixture: &Fixture) -> String {
    let requests = fixture.model.requests();
    let last = requests
        .last()
        .expect("the loop sends the tool results back");
    match last.last() {
        Some(ChatMessage::ToolResults(outcomes)) => outcomes
            .first()
            .expect("every tool call has one outcome")
            .content
            .clone(),
        other => panic!("expected tool results, found {other:?}"),
    }
}

#[test]
fn an_answer_without_tools_streams_text_and_done() {
    let fixture = fixture(vec![ScriptedTurn::reply("Two decisions.")], vault());

    let events = ask(&fixture, "what did I decide?");

    assert_eq!(
        events,
        vec![
            AskEvent::Delta {
                text: "Two decisions.".to_owned()
            },
            AskEvent::Done,
        ]
    );
    assert_eq!(fixture.model.requests().len(), 1);
}

#[test]
fn tool_calls_run_against_the_vault_and_feed_the_next_turn() {
    let fixture = fixture(
        vec![
            ScriptedTurn::reply_with(
                "Looking.",
                vec![call("call_1", "search", json!({"query": "editor rewrite"}))],
            ),
            ScriptedTurn::reply("Done."),
        ],
        vault(),
    );

    let events = ask(&fixture, "what did I decide?");

    assert_eq!(
        events,
        vec![
            AskEvent::Delta {
                text: "Looking.".to_owned()
            },
            AskEvent::Tool {
                name: "search".to_owned(),
                summary: "Searched “editor rewrite”".to_owned()
            },
            AskEvent::Delta {
                text: "Done.".to_owned()
            },
            AskEvent::Done,
        ]
    );

    let requests = fixture.model.requests();
    let first = requests.first().expect("the first turn asks");
    assert_eq!(
        first.first(),
        Some(&ChatMessage::User("what did I decide?".to_owned()))
    );
    let last = requests.last().expect("the loop asks again");
    assert!(matches!(
        last.get(1),
        Some(ChatMessage::Assistant { text, calls, .. })
            if text == "Looking." && calls.first().is_some_and(|call| call.name == "search")
    ));
    let output = tool_result(&fixture);
    assert!(output.contains("[[roadmap]]"), "got {output}");
    assert!(output.contains("projects/roadmap.md"), "got {output}");
}

#[test]
fn the_loop_stops_after_twelve_tool_rounds() {
    let turns: Vec<ScriptedTurn> = (0..12)
        .map(|index| {
            ScriptedTurn::reply_with(
                "",
                vec![call(&format!("call_{index}"), "list_tags", json!({}))],
            )
        })
        .collect();
    let fixture = fixture(turns, vault());

    let events = ask(&fixture, "what did I decide?");

    assert!(
        !events.contains(&AskEvent::Done),
        "the loop answered after giving up"
    );
    assert_eq!(
        events.last(),
        Some(&AskEvent::Failed(Error::IterationsExhausted))
    );
    assert_eq!(fixture.model.requests().len(), 12);
}

#[test]
fn a_provider_failure_is_reported_as_an_event() {
    let fixture = fixture(
        vec![ScriptedTurn::fails(Error::Http {
            status: Some(429),
            message: "slow down".to_owned(),
        })],
        vault(),
    );

    let events = ask(&fixture, "what did I decide?");

    assert_eq!(
        events,
        vec![AskEvent::Failed(Error::Http {
            status: Some(429),
            message: "slow down".to_owned(),
        })]
    );
}

#[test]
fn cancelling_during_a_tool_batch_stops_the_loop_without_a_done_event() {
    let fixture = fixture(
        vec![
            ScriptedTurn::reply_with(
                "",
                vec![
                    call("call_1", "list_tags", json!({})),
                    call("call_2", "find_notes", json!({"name": "road"})),
                ],
            ),
            ScriptedTurn::reply("never reached"),
        ],
        vault(),
    );

    let mut events = Vec::new();
    let result = block_on(fixture.service.ask("req-1", "why?", &mut |event| {
        if matches!(event, AskEvent::Tool { .. }) {
            fixture.service.cancel("req-1");
        }
        events.push(event);
    }));

    result.expect("a cancelled answer reports no error");
    assert_eq!(
        events,
        vec![AskEvent::Tool {
            name: "list_tags".to_owned(),
            summary: "Listed tags".to_owned()
        }]
    );
    assert_eq!(fixture.model.requests().len(), 1);
}

#[test]
fn asking_without_a_configured_model_or_a_question_is_rejected() {
    let unconfigured = AskService::new(
        Arc::new(ScriptedLanguageModel::new(Vec::new())),
        Arc::new(vault()),
        Arc::new(FixedClock::new(NOW)),
    );
    assert_eq!(ask_result(&unconfigured, "why?"), Err(Error::NotConfigured));

    let configured = fixture(vec![ScriptedTurn::reply("never")], vault());
    assert_eq!(
        ask_result(&configured.service, "   "),
        Err(Error::EmptyQuestion)
    );
    assert!(configured.model.requests().is_empty());
}

fn ask_result(service: &AskService, question: &str) -> Result<(), Error> {
    let mut events: Vec<AskEvent> = Vec::new();
    let result = block_on(service.ask("req-1", question, &mut |event| events.push(event)));
    assert!(events.is_empty(), "a rejected question emits nothing");
    result
}

#[test]
fn the_service_is_send_and_sync_and_its_answer_future_is_send() {
    fn assert_send_sync<T: Send + Sync>(_: &T) {}
    fn assert_send<T: Send>(_: &T) {}

    let fixture = fixture(vec![ScriptedTurn::reply("done")], vault());
    assert_send_sync(&fixture.service);
    assert_send(&fixture.service.ask("req-1", "why?", &mut |_event| {}));
}

#[test]
fn listing_models_validates_the_endpoint_before_calling_the_model() {
    let model = ScriptedLanguageModel::new(Vec::new())
        .with_models(vec!["claude-sonnet".to_owned(), "gpt-5".to_owned()]);
    let service = AskService::new(
        Arc::new(model),
        Arc::new(vault()),
        Arc::new(FixedClock::new(NOW)),
    );

    let models = block_on(service.list_models(&config())).expect("the scripted model lists");
    assert_eq!(models, vec!["claude-sonnet".to_owned(), "gpt-5".to_owned()]);

    let unreachable = LlmConfig {
        base_url: "file:///etc".to_owned(),
        ..config()
    };
    assert_eq!(
        block_on(service.list_models(&unreachable)),
        Err(Error::UnsupportedScheme)
    );
}
