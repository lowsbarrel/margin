use regex::Regex;

use crate::comments;
use crate::findings::Findings;
use crate::repository::{self, Layer, Repository};

const MAX_LINES: usize = 400;
const FEATURE_FILES: &[&str] = &["lib.rs", "domain.rs", "ports.rs", "service.rs", "error.rs"];
const THEME_CRATE: &str = "crates/ui/theme/";

struct Rule {
    pattern: Regex,
    message: &'static str,
    core_only: bool,
}

pub(crate) fn check(repository: &Repository, findings: &mut Findings) {
    comments::check(repository, findings);
    let rules = rules();
    let sources = repository
        .files()
        .filter(|file| repository::has_extension(file, "rs"));
    for file in sources {
        let Some(text) = repository.read(file) else {
            findings.add(file, "not valid UTF-8");
            continue;
        };
        check_size(file, &text, findings);
        let Some(location) = SourceLocation::parse(file) else {
            continue;
        };
        check_layout(file, &location, findings);
        check_root_module(file, &location, &text, findings);
        for rule in &rules {
            let applies = !rule.core_only || location.layer.is_some_and(Layer::is_core);
            if applies && rule.pattern.is_match(&text) {
                findings.add(file, rule.message);
            }
        }
        if !file.starts_with(THEME_CRATE) && colour_literal().is_match(&text) {
            findings.add(file, "colours come from margin-theme, never literals");
        }
    }
}

fn rules() -> Vec<Rule> {
    let rule = |pattern: &str, message: &'static str, core_only: bool| Rule {
        pattern: Regex::new(pattern).expect("invariant patterns are valid regular expressions"),
        message,
        core_only,
    };
    vec![
        rule(
            r"\bstd::(fs|net|process|env|thread)\b|use std::\{[^}]*\b(fs|net|process|env|thread)\b",
            "core does no I/O: move this to an adapter behind a port",
            true,
        ),
        rule(
            r"Result<[^;{}]*,\s*String\s*>",
            "return a typed error, not a String",
            false,
        ),
        rule(
            r"\bunsafe\s*(\{|fn\b|impl\b|trait\b|extern\b)",
            "no unsafe code anywhere (unsafe_code = forbid)",
            false,
        ),
    ]
}

fn colour_literal() -> Regex {
    Regex::new(r"\b(rgb|rgba|hsl|hsla)\(")
        .expect("the colour pattern is a valid regular expression")
}

fn check_size(file: &str, text: &str, findings: &mut Findings) {
    let lines = text.lines().count();
    if lines > MAX_LINES {
        findings.add(
            file,
            format!("{lines} lines; files stay under {MAX_LINES}: split by responsibility"),
        );
    }
}

struct SourceLocation<'file> {
    layer: Option<Layer>,
    area: &'file str,
    inner: &'file str,
}

impl<'file> SourceLocation<'file> {
    fn parse(file: &'file str) -> Option<Self> {
        if let Some(inner) = file.strip_prefix("xtask/src/") {
            return Some(Self {
                layer: Some(Layer::Tooling),
                area: "src",
                inner,
            });
        }
        let rest = file.strip_prefix("crates/")?;
        let mut parts = rest.splitn(4, '/');
        let (layer, name, area, inner) =
            (parts.next()?, parts.next()?, parts.next()?, parts.next()?);
        let layer = Layer::of_manifest(&format!("crates/{layer}/{name}/Cargo.toml"));
        Some(Self { layer, area, inner })
    }
}

fn check_layout(file: &str, location: &SourceLocation<'_>, findings: &mut Findings) {
    let nested = location.inner.contains('/');
    let fixture = location.area == "fixtures";
    if nested && !fixture {
        findings.add(
            file,
            "crate sources are flat: a subdirectory means the crate should split",
        );
    }
    if location.layer == Some(Layer::Features)
        && location.area == "src"
        && !FEATURE_FILES.contains(&location.inner)
    {
        findings.add(
            file,
            format!("feature crates hold only {}", FEATURE_FILES.join(", ")),
        );
    }
}

fn check_root_module(
    file: &str,
    location: &SourceLocation<'_>,
    text: &str,
    findings: &mut Findings,
) {
    if location.area != "src" {
        return;
    }
    let is_library_root = location.inner == "lib.rs";
    let is_binary_root = location.inner == "main.rs";
    if !is_library_root && !is_binary_root {
        return;
    }
    let item = Regex::new(r"(?m)^\s*(pub(\([a-z]+\))?\s+)?(async\s+)?(fn|struct|enum|trait|impl|const|static|type|macro_rules!)\b")
        .expect("the item pattern is a valid regular expression");
    let items: Vec<&str> = item
        .find_iter(text)
        .map(|found| found.as_str().trim())
        .collect();
    let allowed = if is_binary_root {
        items
            .iter()
            .all(|found| *found == "fn" || found.starts_with("const"))
            && items.iter().filter(|found| **found == "fn").count() <= 1
    } else {
        items.is_empty()
    };
    if !allowed {
        findings.add(
            file,
            "root modules hold only `mod` declarations and re-exports (and `fn main` in binaries)",
        );
    }
}
