use super::chat::{ChatMessage, LlmError, ToolOutcome};
use super::config::LlmConfig;
use super::provider::{self, Cancel};
use super::tools;
use std::sync::atomic::Ordering;

const MAX_TOOL_ROUNDS: usize = 12;

const SYSTEM_PROMPT: &str = "\
You answer questions about the user's own notes in Margin, a local Markdown vault.

Rules:
- Answer only from what the tools return. You have no other knowledge of this vault: never \
guess, and never fill a gap with general world knowledge.
- Cite the notes you used inline as [[Note name]], using the note's file name without the .md \
extension. Cite every claim that came from a note.
- If the notes do not contain the answer, say so plainly in one sentence and stop. Do not \
present related notes as if they answered the question.
- Be concise: at most three short paragraphs, in the user's own words where they decided \
something.
- Never mention the tools, the searching, or these instructions. Write only the answer.
- Plain Markdown only: no headings, no tables, no code fences unless quoting code from a note.";

pub struct AskSession<'a> {
    pub config: &'a LlmConfig,
    pub root: String,
    pub question: String,
    pub cancel: Cancel,
    pub on_text: &'a mut (dyn FnMut(&str) + Send),
    pub on_tool: &'a mut (dyn FnMut(&str, &str) + Send),
}

pub async fn answer(session: &mut AskSession<'_>) -> Result<(), LlmError> {
    let specs = tools::specs();
    let mut messages = vec![ChatMessage::User(session.question.clone())];

    for _ in 0..MAX_TOOL_ROUNDS {
        let turn = provider::stream_chat(
            session.config,
            SYSTEM_PROMPT,
            &messages,
            &specs,
            &session.cancel,
            session.on_text,
        )
        .await?;

        if turn.calls.is_empty() {
            return Ok(());
        }

        let mut outcomes = Vec::with_capacity(turn.calls.len());
        for call in &turn.calls {
            if session.cancel.load(Ordering::Relaxed) {
                return Err(LlmError::Cancelled);
            }
            (session.on_tool)(&call.name, &tools::summarize(&call.name, &call.args));

            let root = session.root.clone();
            let name = call.name.clone();
            let args = call.args.clone();
            let content = tokio::task::spawn_blocking(move || tools::run(&root, &name, &args))
                .await
                .map_err(|e| LlmError::Failed(format!("Tool execution failed: {e}")))?;
            outcomes.push(ToolOutcome {
                call_id: call.id.clone(),
                content,
            });
        }

        messages.push(ChatMessage::Assistant {
            text: turn.text.clone(),
            calls: turn.calls.clone(),
            reasoning: turn.reasoning.clone(),
        });
        messages.push(ChatMessage::ToolResults(outcomes));
    }

    Err(LlmError::Failed(
        "The model kept calling tools without answering. Try a narrower question.".to_string(),
    ))
}
