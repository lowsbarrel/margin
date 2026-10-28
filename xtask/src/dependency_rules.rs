use std::collections::BTreeMap;
use std::iter;

use toml::{Table, Value};

use crate::findings::Findings;
use crate::repository::{Layer, Repository};

const PURE_CRATES: &[&str] = &[
    "aes-gcm-siv",
    "base64",
    "bip39",
    "hex",
    "hmac",
    "nucleo-matcher",
    "parking_lot",
    "pulldown-cmark",
    "rand",
    "regex",
    "serde",
    "serde_json",
    "sha2",
    "similar",
    "thiserror",
    "unicode-segmentation",
    "zip",
];

const IO_CRATES: &[&str] = &[
    "notify",
    "portable-pty",
    "reqwest",
    "rusqlite",
    "rust-s3",
    "tokio",
    "dirs",
    "rfd",
];

const INHERITED_FIELDS: &[&str] = &[
    "version",
    "edition",
    "rust-version",
    "license",
    "authors",
    "repository",
    "publish",
];

const DEPENDENCY_TABLES: &[(&str, Kind)] = &[
    ("dependencies", Kind::Normal),
    ("build-dependencies", Kind::Normal),
    ("dev-dependencies", Kind::Development),
];

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Kind {
    Normal,
    Development,
}

#[derive(Debug)]
struct Member {
    manifest: String,
    layer: Layer,
    table: Table,
}

pub(crate) fn check(repository: &Repository, findings: &mut Findings) {
    let members = load_members(repository, findings);
    let layers: BTreeMap<&str, Layer> = members
        .values()
        .map(|member| (package_name(&member.table), member.layer))
        .collect();
    for member in members.values() {
        check_inheritance(member, findings);
        let platform_tables = member
            .table
            .get("target")
            .and_then(Value::as_table)
            .into_iter()
            .flat_map(|targets| targets.values().filter_map(Value::as_table));
        let tables = iter::once(&member.table).chain(platform_tables);
        for table in tables {
            for (table_name, kind) in DEPENDENCY_TABLES {
                for (dependency, specification) in dependency_table(table, table_name) {
                    check_dependency(member, dependency, specification, *kind, &layers, findings);
                }
            }
        }
    }
}

fn load_members(repository: &Repository, findings: &mut Findings) -> BTreeMap<String, Member> {
    let mut members = BTreeMap::new();
    let manifests = repository
        .files()
        .filter(|file| file.ends_with("Cargo.toml"));
    for manifest in manifests {
        if manifest == "Cargo.toml" {
            continue;
        }
        let Some(layer) = Layer::of_manifest(manifest) else {
            findings.add(
                manifest,
                "crates live in crates/<layer>/<name>/ (see AGENTS.md)",
            );
            continue;
        };
        let parsed = repository.read(manifest).map(|text| text.parse::<Table>());
        match parsed {
            Some(Ok(table)) => {
                members.insert(
                    manifest.to_owned(),
                    Member {
                        manifest: manifest.to_owned(),
                        layer,
                        table,
                    },
                );
            }
            Some(Err(error)) => findings.add(manifest, format!("invalid TOML: {error}")),
            None => findings.add(manifest, "unreadable manifest"),
        }
    }
    members
}

fn package_name(table: &Table) -> &str {
    table
        .get("package")
        .and_then(|package| package.get("name"))
        .and_then(Value::as_str)
        .unwrap_or("")
}

fn check_inheritance(member: &Member, findings: &mut Findings) {
    let package = member.table.get("package").and_then(Value::as_table);
    for field in INHERITED_FIELDS {
        let inherited = package
            .and_then(|package| package.get(*field))
            .and_then(|value| value.get("workspace"))
            .and_then(Value::as_bool)
            == Some(true);
        if !inherited {
            findings.add(
                &member.manifest,
                format!("package.{field} must be `{field}.workspace = true`"),
            );
        }
    }
    let has_description = package
        .and_then(|package| package.get("description"))
        .is_some();
    if !has_description {
        findings.add(&member.manifest, "package.description is required");
    }
    let lints_inherited = member
        .table
        .get("lints")
        .and_then(|lints| lints.get("workspace"))
        .and_then(Value::as_bool)
        == Some(true);
    if !lints_inherited {
        findings.add(&member.manifest, "[lints] must be `workspace = true`");
    }
}

fn dependency_table<'table>(
    table: &'table Table,
    name: &str,
) -> impl Iterator<Item = (&'table str, &'table Value)> {
    table
        .get(name)
        .and_then(Value::as_table)
        .into_iter()
        .flatten()
        .map(|(key, value)| (key.as_str(), value))
}

fn check_dependency(
    member: &Member,
    dependency: &str,
    specification: &Value,
    kind: Kind,
    layers: &BTreeMap<&str, Layer>,
    findings: &mut Findings,
) {
    let from_workspace = specification.get("workspace").and_then(Value::as_bool) == Some(true);
    if !from_workspace {
        findings.add(
            &member.manifest,
            format!("`{dependency}` must be `{dependency}.workspace = true`"),
        );
    }
    match layers.get(dependency) {
        Some(target) => check_internal(member, dependency, *target, kind, findings),
        None => check_external(member, dependency, kind, findings),
    }
}

fn check_internal(
    member: &Member,
    dependency: &str,
    target: Layer,
    kind: Kind,
    findings: &mut Findings,
) {
    if target == Layer::Testing && kind == Kind::Normal && member.layer != Layer::Testing {
        findings.add(
            &member.manifest,
            format!("`{dependency}` is test support: use it as a dev-dependency"),
        );
        return;
    }
    if target == Layer::Testing && kind == Kind::Development {
        return;
    }
    let allowed = match member.layer {
        Layer::Shared => matches!(target, Layer::Shared),
        Layer::Features | Layer::Testing => matches!(target, Layer::Shared | Layer::Features),
        Layer::Adapters => matches!(target, Layer::Shared | Layer::Features),
        Layer::Ui => matches!(target, Layer::Shared | Layer::Features | Layer::Ui),
        Layer::Apps => true,
        Layer::Tooling => false,
    };
    if !allowed {
        findings.add(
            &member.manifest,
            format!(
                "{:?} crates may not depend on `{dependency}` ({target:?})",
                member.layer
            ),
        );
    }
}

fn check_external(member: &Member, dependency: &str, kind: Kind, findings: &mut Findings) {
    let pure_only = member.layer.is_core() || member.layer == Layer::Testing;
    let is_test_tool = kind == Kind::Development && member.layer == Layer::Testing;
    if pure_only && !is_test_tool && !PURE_CRATES.contains(&dependency) {
        findings.add(
            &member.manifest,
            format!("core crates may only use allowlisted pure crates; `{dependency}` is not (xtask/src/dependency_rules.rs)"),
        );
    }
    if member.layer == Layer::Ui && IO_CRATES.contains(&dependency) {
        findings.add(
            &member.manifest,
            format!("UI crates call use cases, never `{dependency}` directly"),
        );
    }
}
