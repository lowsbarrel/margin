use margin_kernel::RelativePath;
use margin_markdown::WikiLink;

use crate::vault_index::VaultIndex;
use crate::vault_paths::{
    decode_destination, extension_of, find_variant, has_scheme, is_note_path, joined,
    localfile_path, normalized, note_dir,
};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum LinkRef<'a> {
    Wiki(&'a WikiLink),
    Destination(&'a str),
    Autolink(&'a str),
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Resolved {
    Note {
        path: RelativePath,
        anchor: Option<String>,
    },
    MissingNote {
        path_to_create: RelativePath,
    },
    Attachment(RelativePath),
    External(String),
    OutsideVault,
    Unsupported,
}

#[must_use]
pub fn resolve_link(index: &VaultIndex, note: &RelativePath, link: LinkRef<'_>) -> Resolved {
    match link {
        LinkRef::Wiki(wiki) => resolve_wiki(index, note, wiki),
        LinkRef::Destination(destination) => resolve_destination(index, note, destination),
        LinkRef::Autolink(text) => {
            let href = web_href(text);
            if opens_in_browser(&href) {
                Resolved::External(href)
            } else {
                Resolved::Unsupported
            }
        }
    }
}

#[must_use]
pub fn web_href(text: &str) -> String {
    let raw = text.strip_prefix('<').unwrap_or(text);
    let raw = raw.strip_suffix('>').unwrap_or(raw);
    if has_scheme(raw) {
        raw.to_owned()
    } else if raw.contains('@') && !raw.contains('/') {
        format!("mailto:{raw}")
    } else {
        format!("https://{raw}")
    }
}

fn opens_in_browser(href: &str) -> bool {
    ["http:", "https:", "mailto:"]
        .iter()
        .any(|scheme| href.starts_with(scheme))
}

fn resolve_wiki(index: &VaultIndex, note: &RelativePath, wiki: &WikiLink) -> Resolved {
    let target = wiki.target.trim();
    if target.is_empty() {
        return Resolved::Unsupported;
    }
    let anchor = wiki
        .heading
        .clone()
        .or_else(|| wiki.block.as_ref().map(|block| format!("^{block}")));
    let extension = extension_of(target);
    let named_note = extension == "md" || extension == "canvas";
    let variants = if named_note {
        vec![target.to_owned()]
    } else {
        vec![
            format!("{target}.md"),
            format!("{target}.canvas"),
            target.to_owned(),
        ]
    };
    if let Some(path) = find_variant(index, note, &variants) {
        return if is_note_path(&path) {
            Resolved::Note { path, anchor }
        } else {
            Resolved::Attachment(path)
        };
    }
    let name = if named_note {
        target.to_owned()
    } else {
        format!("{target}.md")
    };
    let created = if target.contains('/') {
        normalized(&name)
    } else {
        normalized(&joined(&note_dir(note), &name))
    };
    created.map_or(Resolved::OutsideVault, |path_to_create| {
        Resolved::MissingNote { path_to_create }
    })
}

fn resolve_destination(index: &VaultIndex, note: &RelativePath, destination: &str) -> Resolved {
    let raw = destination.trim();
    let raw = raw.strip_prefix('<').unwrap_or(raw);
    let raw = raw.strip_suffix('>').unwrap_or(raw);
    if raw.is_empty() || raw.starts_with('#') {
        return Resolved::Unsupported;
    }
    if opens_in_browser(raw) {
        return Resolved::External(raw.to_owned());
    }
    if let Some(absolute) = localfile_path(raw) {
        return index
            .relative(&absolute)
            .map_or(Resolved::OutsideVault, |path| {
                let exists = index.exists(&path);
                classified(path, None, exists)
            });
    }
    if has_scheme(raw) {
        return Resolved::Unsupported;
    }
    let (path_part, anchor) = match raw.split_once('#') {
        Some((path, anchor)) => (path, Some(decode_destination(anchor))),
        None => (raw, None),
    };
    let decoded = decode_destination(path_part);
    let candidates: Vec<RelativePath> = if decoded.starts_with('/') {
        vec![normalized(&decoded)]
    } else {
        vec![
            normalized(&joined(&note_dir(note), &decoded)),
            normalized(&decoded),
        ]
    }
    .into_iter()
    .flatten()
    .collect();
    let Some(first) = candidates.first() else {
        return Resolved::OutsideVault;
    };
    let with_note_extension = |path: &RelativePath| {
        if extension_of(path.file_name()).is_empty() {
            path.with_file_name(&format!("{}.md", path.file_name()))
                .ok()
        } else {
            None
        }
    };
    for candidate in &candidates {
        for variant in [Some(candidate.clone()), with_note_extension(candidate)]
            .into_iter()
            .flatten()
        {
            if index.exists(&variant) {
                return classified(variant, anchor, true);
            }
        }
    }
    let missing = with_note_extension(first).unwrap_or_else(|| first.clone());
    classified(missing, anchor, false)
}

fn classified(path: RelativePath, anchor: Option<String>, exists: bool) -> Resolved {
    if !is_note_path(&path) {
        return Resolved::Attachment(path);
    }
    if exists {
        Resolved::Note { path, anchor }
    } else {
        Resolved::MissingNote {
            path_to_create: path,
        }
    }
}

#[cfg(test)]
mod tests {
    use std::path::PathBuf;

    use super::*;

    fn path(raw: &str) -> RelativePath {
        RelativePath::new(raw).expect("test paths are valid")
    }

    fn index() -> VaultIndex {
        VaultIndex::new(
            PathBuf::from("/vault"),
            path("attachments"),
            [
                path("Projects/Roadmap.md"),
                path("Projects/Plan.md"),
                path("Plan.md"),
                path("Board.canvas"),
                path("attachments/report.pdf"),
                path("docs/space name.md"),
            ],
        )
    }

    fn wiki(target: &str, heading: Option<&str>) -> WikiLink {
        WikiLink {
            target: target.to_owned(),
            heading: heading.map(str::to_owned),
            block: None,
            alias: None,
        }
    }

    fn resolve(note: &str, link: LinkRef<'_>) -> Resolved {
        resolve_link(&index(), &path(note), link)
    }

    #[test]
    fn wiki_links_prefer_the_note_folder_then_match_by_name() {
        let plan = wiki("Plan", Some("Goals"));
        assert_eq!(
            resolve("Projects/Today.md", LinkRef::Wiki(&plan)),
            Resolved::Note {
                path: path("Projects/Plan.md"),
                anchor: Some(String::from("Goals")),
            }
        );
        assert_eq!(
            resolve("Inbox/Today.md", LinkRef::Wiki(&plan)),
            Resolved::Note {
                path: path("Plan.md"),
                anchor: Some(String::from("Goals")),
            }
        );
        let roadmap = wiki("roadmap", None);
        assert_eq!(
            resolve("Inbox/Today.md", LinkRef::Wiki(&roadmap)),
            Resolved::Note {
                path: path("Projects/Roadmap.md"),
                anchor: None,
            }
        );
        let board = wiki("Board", None);
        assert_eq!(
            resolve("Today.md", LinkRef::Wiki(&board)),
            Resolved::Note {
                path: path("Board.canvas"),
                anchor: None,
            }
        );
        let report = wiki("report.pdf", None);
        assert_eq!(
            resolve("Today.md", LinkRef::Wiki(&report)),
            Resolved::Attachment(path("attachments/report.pdf"))
        );
    }

    #[test]
    fn a_missing_wiki_note_is_created_beside_the_note() {
        let idea = wiki("Idea", None);
        assert_eq!(
            resolve("Inbox/Today.md", LinkRef::Wiki(&idea)),
            Resolved::MissingNote {
                path_to_create: path("Inbox/Idea.md"),
            }
        );
        let nested = wiki("Area/Idea", None);
        assert_eq!(
            resolve("Inbox/Today.md", LinkRef::Wiki(&nested)),
            Resolved::MissingNote {
                path_to_create: path("Area/Idea.md"),
            }
        );
        let escape = wiki("../../Idea", None);
        assert_eq!(
            resolve("Inbox/Today.md", LinkRef::Wiki(&escape)),
            Resolved::OutsideVault
        );
    }

    #[test]
    fn markdown_destinations_decode_and_resolve_from_the_note_folder() {
        assert_eq!(
            resolve("docs/Today.md", LinkRef::Destination("space%20name.md#Top")),
            Resolved::Note {
                path: path("docs/space name.md"),
                anchor: Some(String::from("Top")),
            }
        );
        assert_eq!(
            resolve("Projects/Today.md", LinkRef::Destination("<Plan>")),
            Resolved::Note {
                path: path("Projects/Plan.md"),
                anchor: None,
            }
        );
        assert_eq!(
            resolve("Today.md", LinkRef::Destination("/Projects/Roadmap.md")),
            Resolved::Note {
                path: path("Projects/Roadmap.md"),
                anchor: None,
            }
        );
        assert_eq!(
            resolve("a/Today.md", LinkRef::Destination("New.md")),
            Resolved::MissingNote {
                path_to_create: path("a/New.md"),
            }
        );
        assert_eq!(
            resolve("Today.md", LinkRef::Destination("attachments/report.pdf")),
            Resolved::Attachment(path("attachments/report.pdf"))
        );
    }

    #[test]
    fn links_outside_the_vault_are_refused() {
        assert_eq!(
            resolve("a/Today.md", LinkRef::Destination("../../etc/passwd")),
            Resolved::OutsideVault
        );
        assert_eq!(
            resolve(
                "Today.md",
                LinkRef::Destination("localfile://localhost/etc/passwd")
            ),
            Resolved::OutsideVault
        );
        assert_eq!(
            resolve(
                "Today.md",
                LinkRef::Destination("localfile://localhost/vault/Plan.md")
            ),
            Resolved::Note {
                path: path("Plan.md"),
                anchor: None,
            }
        );
    }

    #[test]
    fn urls_open_externally_and_other_schemes_are_unsupported() {
        assert_eq!(
            resolve("Today.md", LinkRef::Destination("https://example.com")),
            Resolved::External(String::from("https://example.com"))
        );
        assert_eq!(
            resolve("Today.md", LinkRef::Autolink("www.example.com")),
            Resolved::External(String::from("https://www.example.com"))
        );
        assert_eq!(
            resolve("Today.md", LinkRef::Autolink("me@example.com")),
            Resolved::External(String::from("mailto:me@example.com"))
        );
        assert_eq!(
            resolve("Today.md", LinkRef::Destination("#heading")),
            Resolved::Unsupported
        );
        assert_eq!(
            resolve("Today.md", LinkRef::Destination("obsidian://open?x")),
            Resolved::Unsupported
        );
    }
}
