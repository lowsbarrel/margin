use std::sync::Arc;

use futures_channel::mpsc;
use futures_util::StreamExt;
use gpui_kit::{Context, Task};
use margin_ask::{AskEvent, AskService};
use margin_workbench::run_async;

use crate::spotlight::Spotlight;

#[derive(Debug, Default)]
pub(crate) struct AskSession {
    configured: bool,
    question: String,
    steps: Vec<String>,
    answer: String,
    error: Option<String>,
    request: Option<String>,
    issued: u64,
}

impl AskSession {
    pub(crate) fn reset(&mut self) {
        self.question.clear();
        self.steps.clear();
        self.answer.clear();
        self.error = None;
        self.request = None;
    }

    pub(crate) fn set_configured(&mut self, configured: bool) {
        self.configured = configured;
    }

    pub(crate) const fn is_configured(&self) -> bool {
        self.configured
    }

    pub(crate) const fn is_running(&self) -> bool {
        self.request.is_some()
    }

    pub(crate) fn question(&self) -> &str {
        &self.question
    }

    pub(crate) fn steps(&self) -> &[String] {
        &self.steps
    }

    pub(crate) fn answer(&self) -> &str {
        &self.answer
    }

    pub(crate) fn error(&self) -> Option<&str> {
        self.error.as_deref()
    }

    pub(crate) fn begin(&mut self, question: &str) -> Option<String> {
        if self.is_running() || question.is_empty() {
            return None;
        }
        self.reset();
        self.issued += 1;
        let request = format!("spotlight-{}", self.issued);
        question.clone_into(&mut self.question);
        self.request = Some(request.clone());
        Some(request)
    }

    pub(crate) fn apply(&mut self, request: &str, event: AskEvent) -> bool {
        if self.request.as_deref() != Some(request) {
            return false;
        }
        match event {
            AskEvent::Tool { summary, .. } => self.steps.push(summary),
            AskEvent::Delta { text } => self.answer.push_str(&text),
            AskEvent::Done => self.request = None,
            AskEvent::Failed(error) => {
                self.error = Some(error.to_string());
                self.request = None;
            }
        }
        true
    }

    pub(crate) fn finish(&mut self, request: &str, failure: Option<String>) -> bool {
        if self.request.as_deref() != Some(request) {
            return false;
        }
        if failure.is_some() {
            self.error = failure;
        }
        self.request = None;
        true
    }

    pub(crate) fn cancel(&mut self) -> Option<String> {
        self.request.take()
    }
}

pub(crate) fn stream(
    service: Arc<AskService>,
    request: String,
    question: String,
    session: fn(&mut Spotlight) -> &mut AskSession,
    cx: &Context<'_, Spotlight>,
) -> Task<()> {
    let (sender, mut receiver) = mpsc::unbounded();
    let id = request.clone();
    let run = run_async(cx, async move {
        let mut forward = |event| drop(sender.unbounded_send(event));
        service.ask(&id, &question, &mut forward).await
    });
    cx.spawn(async move |this, cx| {
        while let Some(event) = receiver.next().await {
            let applied = this.update(cx, |this, cx| {
                if session(this).apply(&request, event) {
                    cx.notify();
                }
            });
            if applied.is_err() {
                return;
            }
        }
        let failure = run.await.err().map(|error| error.to_string());
        drop(this.update(cx, |this, cx| {
            if session(this).finish(&request, failure) {
                cx.notify();
            }
        }));
    })
}

#[cfg(test)]
mod tests {
    use margin_ask::Error;

    use super::*;

    #[test]
    fn streams_steps_and_answer_until_done() {
        let mut session = AskSession::default();
        let request = session.begin("why").expect("idle session starts");
        assert!(session.begin("again").is_none());
        session.apply(
            &request,
            AskEvent::Tool {
                name: "search".to_owned(),
                summary: "Searched “why”".to_owned(),
            },
        );
        session.apply(
            &request,
            AskEvent::Delta {
                text: "Because ".to_owned(),
            },
        );
        session.apply(
            &request,
            AskEvent::Delta {
                text: "[[Plan]]".to_owned(),
            },
        );
        session.apply(&request, AskEvent::Done);
        assert_eq!(session.steps(), ["Searched “why”"]);
        assert_eq!(session.answer(), "Because [[Plan]]");
        assert!(!session.is_running());
    }

    #[test]
    fn a_cancelled_request_ignores_late_events() {
        let mut session = AskSession::default();
        let request = session.begin("why").expect("idle session starts");
        assert_eq!(session.cancel().as_deref(), Some(request.as_str()));
        assert!(!session.apply(
            &request,
            AskEvent::Delta {
                text: "late".to_owned()
            }
        ));
        assert!(session.answer().is_empty());
        let next = session.begin("again").expect("cancelled session restarts");
        assert_ne!(next, request);
    }

    #[test]
    fn failures_end_the_request_with_an_error() {
        let mut session = AskSession::default();
        let request = session.begin("why").expect("idle session starts");
        session.apply(&request, AskEvent::Failed(Error::NotConfigured));
        assert!(!session.is_running());
        assert!(session.error().is_some());
    }
}
