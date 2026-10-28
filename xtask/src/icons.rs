use std::collections::BTreeSet;

use regex::Regex;

use crate::findings::Findings;
use crate::repository::{Repository, has_extension};

const EMBEDDED_ICONS: &str = "crates/apps/desktop/src/icons.rs";

pub(crate) fn check(repository: &Repository, findings: &mut Findings) {
    let usage = Regex::new("IconName::([A-Z][A-Za-z0-9]*)").expect("the icon pattern is valid");
    let Some(embedded) = repository.read(EMBEDDED_ICONS) else {
        findings.add(EMBEDDED_ICONS, "the curated icon set is missing");
        return;
    };
    let embedded: BTreeSet<&str> = embedded
        .lines()
        .map(|line| line.trim().trim_end_matches(','))
        .collect();
    let sources = repository
        .files()
        .filter(|file| file.starts_with("crates/") && has_extension(file, "rs"));
    for file in sources {
        let Some(text) = repository.read(file) else {
            continue;
        };
        let missing: BTreeSet<&str> = usage
            .captures_iter(&text)
            .filter_map(|capture| capture.get(1))
            .map(|name| name.as_str())
            .filter(|name| !embedded.contains(name))
            .collect();
        for name in missing {
            findings.add(
                file,
                format!("icon `{name}` is used but not embedded: add it to {EMBEDDED_ICONS}"),
            );
        }
    }
}
