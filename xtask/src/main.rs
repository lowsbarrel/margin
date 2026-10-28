mod comments;
mod dependency_rules;
mod findings;
mod invariants;
mod messages;
mod repository;
mod secrets;

use std::env;
use std::process::ExitCode;

use findings::Findings;
use repository::Repository;

const USAGE: &str =
    "usage: cargo xtask <check | check-deps | check-invariants | check-messages | check-secrets>";

fn main() -> ExitCode {
    let command = env::args().nth(1).unwrap_or_default();
    let repository = match Repository::open() {
        Ok(repository) => repository,
        Err(error) => return findings::fail(&format!("cannot read the repository: {error}")),
    };
    let mut findings = Findings::default();
    match command.as_str() {
        "check" => {
            dependency_rules::check(&repository, &mut findings);
            invariants::check(&repository, &mut findings);
            messages::check(&repository, &mut findings);
            secrets::check(&repository, &mut findings);
        }
        "check-deps" => dependency_rules::check(&repository, &mut findings),
        "check-invariants" => invariants::check(&repository, &mut findings),
        "check-messages" => messages::check(&repository, &mut findings),
        "check-secrets" => secrets::check(&repository, &mut findings),
        _ => return findings::fail(USAGE),
    }
    findings.report(&command)
}
