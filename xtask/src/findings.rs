use std::io::{self, Write};
use std::process::ExitCode;

#[derive(Debug)]
struct Finding {
    file: String,
    message: String,
}

#[derive(Debug, Default)]
pub(crate) struct Findings {
    items: Vec<Finding>,
}

impl Findings {
    pub(crate) fn add(&mut self, file: &str, message: impl Into<String>) {
        self.items.push(Finding {
            file: file.to_owned(),
            message: message.into(),
        });
    }

    pub(crate) fn report(mut self, command: &str) -> ExitCode {
        if self.items.is_empty() {
            let mut stdout = io::stdout().lock();
            return match writeln!(stdout, "ok: {command}") {
                Ok(()) => ExitCode::SUCCESS,
                Err(_) => ExitCode::FAILURE,
            };
        }
        self.items
            .sort_by(|left, right| (&left.file, &left.message).cmp(&(&right.file, &right.message)));
        let mut stderr = io::stderr().lock();
        for finding in &self.items {
            if writeln!(stderr, "{}: {}", finding.file, finding.message).is_err() {
                break;
            }
        }
        ExitCode::FAILURE
    }
}

pub(crate) fn fail(message: &str) -> ExitCode {
    let mut stderr = io::stderr().lock();
    let _ignored: io::Result<()> = writeln!(stderr, "{message}");
    ExitCode::FAILURE
}
