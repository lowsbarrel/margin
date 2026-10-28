use std::sync::Arc;
use std::time::Duration;

use margin_ask::{ApiFormat, AskEvent, AskService, ChatMessage, LlmConfig, ToolCall};
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

fn vault() -> MemoryVaultReader {
    let vault = MemoryVaultReader::new();
    vault.insert(
        "projects/roadmap.md",
        "#roadmap\nShip the **editor** rewrite.\nDeferred: sync conflicts.\n",
    );
    vault.insert("projects/beta.md", "Links to [[roadmap]].\n");
    vault.insert("journal/today.md", "#journal\nEditor work again.\n");
    vault
}

fn tool_output(name: &str, args: Value) -> String {
    tool_output_in(vault(), name, args)
}

fn tool_output_in(vault: MemoryVaultReader, name: &str, args: Value) -> String {
    let model = ScriptedLanguageModel::new(vec![
        ScriptedTurn::reply_with("", vec![call("call_1", name, args)]),
        ScriptedTurn::reply("done"),
    ]);
    let service = AskService::new(
        Arc::new(model.clone()),
        Arc::new(vault),
        Arc::new(FixedClock::new(NOW)),
    );
    service
        .configure(config())
        .expect("the test config is valid");
    ask_events(&service, "what did I decide?");
    tool_result(&model)
}

fn ask_events(service: &AskService, question: &str) -> Vec<AskEvent> {
    let mut events = Vec::new();
    block_on(service.ask("req-1", question, &mut |event| {
        events.push(event);
    }))
    .expect("a failed answer arrives as an event, not as an error");
    events
}

fn tool_result(model: &ScriptedLanguageModel) -> String {
    let requests = model.requests();
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
fn search_finds_notes_and_cites_them_by_name() {
    let output = tool_output("search", json!({"query": "editor rewrite"}));
    assert!(output.contains("[[roadmap]]"), "got {output}");
    assert!(output.contains("projects/roadmap.md"), "got {output}");
    assert!(!output.contains("journal/today.md"), "got {output}");

    let output = tool_output("search", json!({"query": "editor", "mode": "any"}));
    assert!(output.contains("journal/today.md"), "got {output}");

    let output = tool_output(
        "search",
        json!({"query": "editor rewrite", "mode": "phrase"}),
    );
    assert!(output.contains("[[roadmap]]"), "got {output}");

    let reversed = tool_output(
        "search",
        json!({"query": "rewrite editor", "mode": "phrase"}),
    );
    assert!(!reversed.contains("[[roadmap]]"), "got {reversed}");
}

#[test]
fn read_note_caps_the_range_and_never_escapes_the_vault() {
    let output = tool_output("read_note", json!({"path": "projects/roadmap.md"}));
    assert!(
        output.contains("Ship the **editor** rewrite."),
        "got {output}"
    );
    assert!(output.contains("lines 1-"), "got {output}");

    let ranged = tool_output(
        "read_note",
        json!({"path": "projects/roadmap.md", "from_line": 2, "to_line": 2}),
    );
    assert!(ranged.contains("Ship the"), "got {ranged}");
    assert!(!ranged.contains("Deferred:"), "got {ranged}");

    let escaped = tool_output("read_note", json!({"path": "../escape.md"}));
    assert_eq!(escaped, "../escape.md escapes the vault");
}

#[test]
fn grep_reports_line_numbers_and_respects_a_folder() {
    let output = tool_output("grep", json!({"pattern": "Editor", "folder": "journal"}));
    assert!(output.starts_with("journal/today.md:2:"), "got {output}");
    assert!(!output.contains("roadmap.md"), "got {output}");

    let output = tool_output("grep", json!({"pattern": "^Deferred"}));
    assert!(output.starts_with("projects/roadmap.md:3:"), "got {output}");

    let output = tool_output("grep", json!({"pattern": "("}));
    assert!(
        output.contains("Invalid regular expression"),
        "got {output}"
    );
}

#[test]
fn tags_links_and_recents_come_from_the_vault() {
    let output = tool_output("list_tags", json!({}));
    assert!(output.contains("#roadmap (1 notes)"), "got {output}");
    assert!(output.contains("#journal (1 notes)"), "got {output}");

    let output = tool_output("notes_with_tag", json!({"tag": "Roadmap"}));
    assert!(output.contains("[[roadmap]]"), "got {output}");
    assert!(!output.contains("today.md"), "got {output}");

    let output = tool_output("backlinks", json!({"path": "projects/roadmap.md"}));
    assert!(output.contains("[[beta]]"), "got {output}");

    let output = tool_output("find_notes", json!({"name": "road"}));
    assert!(output.contains("[[roadmap]]"), "got {output}");
}

#[test]
fn tags_are_listed_most_used_first() {
    let vault = MemoryVaultReader::new();
    vault.insert("a.md", "#shared #rare\n");
    vault.insert("b.md", "#shared\n");

    let output = tool_output_in(vault, "list_tags", json!({}));

    assert_eq!(output, "#shared (2 notes)\n#rare (1 notes)\n");
}

#[test]
fn recent_notes_answers_from_the_clock_and_the_modified_stamps() {
    let vault = MemoryVaultReader::new();
    vault.insert_modified(
        "projects/roadmap.md",
        "#roadmap\n",
        NOW.saturating_sub(Duration::from_hours(1)),
    );
    vault.insert_modified(
        "journal/today.md",
        "#journal\n",
        NOW.saturating_sub(Duration::from_hours(48)),
    );

    let output = tool_output_in(
        vault.clone(),
        "recent_notes",
        json!({"days": 1, "limit": 5}),
    );
    assert_eq!(output, "[[roadmap]] (projects/roadmap.md, today)\n");

    let output = tool_output_in(vault, "recent_notes", json!({"days": 3, "limit": 5}));
    assert_eq!(
        output,
        "[[roadmap]] (projects/roadmap.md, today)\n[[today]] (journal/today.md, 2 days ago)\n"
    );
}

#[test]
fn a_missing_argument_or_an_unknown_tool_is_reported_to_the_model() {
    let output = tool_output("read_note", json!({}));
    assert!(
        output.contains("Missing required argument: path"),
        "got {output}"
    );

    let output = tool_output("search", json!({"query": "   "}));
    assert!(
        output.contains("Missing required argument: query"),
        "got {output}"
    );

    let output = tool_output("nonsense", json!({}));
    assert_eq!(output, "Unknown tool: nonsense");
}

#[test]
fn a_long_note_is_truncated_rather_than_flooding_the_prompt() {
    let vault = MemoryVaultReader::new();
    vault.insert("projects/long.md", &"x".repeat(20_000));

    let output = tool_output_in(vault, "read_note", json!({"path": "projects/long.md"}));

    assert!(output.starts_with("projects/long.md (lines 1-1 of 1)"));
    assert!(output.ends_with("… (truncated)"), "got {}", output.len());
    assert!(output.len() <= 8_100, "not capped: {}", output.len());
}

#[test]
fn every_advertised_tool_is_dispatched() {
    let model = ScriptedLanguageModel::new(vec![ScriptedTurn::reply("done")]);
    let service = AskService::new(
        Arc::new(model.clone()),
        Arc::new(vault()),
        Arc::new(FixedClock::new(NOW)),
    );
    service
        .configure(config())
        .expect("the test config is valid");
    let events = ask_events(&service, "what did I decide?");
    assert!(events.contains(&AskEvent::Done));

    let names: Vec<String> = model
        .tools()
        .iter()
        .map(|tool| tool.name.to_owned())
        .collect();
    assert_eq!(
        names,
        vec![
            "search",
            "grep",
            "find_notes",
            "read_note",
            "list_tags",
            "notes_with_tag",
            "backlinks",
            "recent_notes"
        ]
    );

    for name in names {
        let output = tool_output(&name, json!({}));
        assert!(!output.contains("Unknown tool"), "{name} is not dispatched");
    }
}
