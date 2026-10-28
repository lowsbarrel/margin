use regex::Regex;

use crate::findings::Findings;
use crate::repository::Repository;

const SKIPPED_SUFFIXES: &[&str] = &[
    "Cargo.lock",
    ".png",
    ".jpg",
    ".jpeg",
    ".gif",
    ".ico",
    ".icns",
    ".svg",
    ".woff",
    ".woff2",
    ".ttf",
    ".otf",
    ".pdf",
];
const THIS_FILE: &str = "xtask/src/secrets.rs";

const PATTERNS: &[(&str, &str)] = &[
    (
        "-----BEGIN (?:RSA |EC |OPENSSH |DSA )?PRIVATE KEY-----",
        "private key",
    ),
    (
        "untrusted comment: (?:minisign|rsign) encrypted secret key",
        "minisign secret key",
    ),
    ("AKIA[0-9A-Z]{16}", "AWS access key id"),
    ("GOCSPX-[A-Za-z0-9_-]{20,}", "Google OAuth client secret"),
    ("xox[baprs]-[A-Za-z0-9-]{10,}", "Slack token"),
    ("gh[pousr]_[A-Za-z0-9]{30,}", "GitHub token"),
    (
        "sk-(?:ant-|proj-)?[A-Za-z0-9_-]{32,}",
        "AI provider API key",
    ),
    (
        r#"(?i)mnemonic\s*[:=]\s*["'][a-z]+(?: [a-z]+){11}["']"#,
        "BIP-39 mnemonic",
    ),
    (
        r#"(?i)(?:aws_)?secret_access_key\s*[:=]\s*["']?[A-Za-z0-9/+=]{30,}"#,
        "S3 secret access key",
    ),
    (
        r#"(?i)(?:signing|packager)[a-z_]*private_key\s*[:=]\s*["']?[A-Za-z0-9/+=]{40,}"#,
        "signing key",
    ),
];

pub(crate) fn check(repository: &Repository, findings: &mut Findings) {
    let patterns: Vec<(Regex, &str)> = PATTERNS
        .iter()
        .map(|(pattern, label)| {
            (
                Regex::new(pattern).expect("secret patterns are valid"),
                *label,
            )
        })
        .collect();
    let candidates = repository.files().filter(|file| {
        *file != THIS_FILE && !SKIPPED_SUFFIXES.iter().any(|suffix| file.ends_with(suffix))
    });
    for file in candidates {
        let Some(text) = repository.read(file) else {
            continue;
        };
        for (pattern, label) in &patterns {
            if pattern.is_match(&text) {
                findings.add(file, format!("looks like a {label}: rotate it if real, refine xtask/src/secrets.rs if not"));
            }
        }
    }
}
