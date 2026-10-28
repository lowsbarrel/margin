use gpui_kit::assets::IconName;
use margin_editor_commands::{SlashCommand, fuzzy_match};
use margin_kernel::RelativePath;
use margin_markdown::Span;
use margin_search::{FuzzyEntry, filter_files};

use crate::completion_item::CompletionItem;
use crate::trigger::{Trigger, TriggerKind};

const LINK_LIMIT: usize = 25;
const TAG_LIMIT: usize = 30;
const IMAGE_EXTENSIONS: [&str; 11] = [
    "png", "jpg", "jpeg", "gif", "webp", "svg", "bmp", "avif", "ico", "tiff", "tif",
];

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Completion {
    Slash {
        trigger: Span,
        command: SlashCommand,
    },
    Insert {
        replace: Span,
        text: String,
        cursor_after: usize,
    },
}

pub(crate) const fn slash_icon(command: SlashCommand) -> IconName {
    match command {
        SlashCommand::Heading1 => IconName::Heading1,
        SlashCommand::Heading2 => IconName::Heading2,
        SlashCommand::Heading3 => IconName::Heading3,
        SlashCommand::BulletList => IconName::List,
        SlashCommand::NumberedList => IconName::ListOrdered,
        SlashCommand::TodoList => IconName::ListTodo,
        SlashCommand::Quote => IconName::Quote,
        SlashCommand::CodeBlock => IconName::SquareCode,
        SlashCommand::Table => IconName::Table,
        SlashCommand::Callout => IconName::Lightbulb,
        SlashCommand::MathBlock => IconName::Sigma,
        SlashCommand::Mermaid => IconName::Workflow,
        SlashCommand::Divider => IconName::Minus,
        SlashCommand::Link => IconName::Link,
        SlashCommand::EmbedNote => IconName::FileText,
        SlashCommand::Date => IconName::Calendar,
    }
}

pub(crate) fn slash_item(
    trigger: &Trigger,
    command: SlashCommand,
    label: String,
    description: String,
) -> CompletionItem {
    let completion = Completion::Slash {
        trigger: trigger.span,
        command,
    };
    CompletionItem::new(slash_icon(command), label, &trigger.query, completion)
        .described(description)
}

fn link_completion(trigger: &Trigger, name: &str) -> Completion {
    let closed = matches!(
        trigger.kind,
        TriggerKind::WikiLink { closed: true, .. } | TriggerKind::Heading { closed: true, .. }
    );
    let end = if closed {
        trigger.span.end + 2
    } else {
        trigger.span.end
    };
    let text = format!("{name}]]");
    Completion::Insert {
        replace: Span::new(trigger.span.start, end),
        cursor_after: text.len(),
        text,
    }
}

fn has_extension_in(path: &RelativePath, extensions: &[&str]) -> bool {
    path.extension().is_some_and(|found| {
        extensions
            .iter()
            .any(|extension| found.eq_ignore_ascii_case(extension))
    })
}

pub(crate) fn is_note(path: &RelativePath) -> bool {
    has_extension_in(path, &["md", "canvas"])
}

fn link_name(name: &str) -> &str {
    for extension in [".md", ".canvas"] {
        let cut = name.len().saturating_sub(extension.len());
        if name.is_char_boundary(cut) && name[cut..].eq_ignore_ascii_case(extension) {
            return &name[..cut];
        }
    }
    name
}

#[must_use]
pub(crate) fn link_items(files: &[RelativePath], trigger: &Trigger) -> Vec<CompletionItem> {
    let (notes, attachments): (Vec<FuzzyEntry>, Vec<FuzzyEntry>) = files
        .iter()
        .map(|path| FuzzyEntry {
            name: path.file_name().to_owned(),
            path: path.clone(),
        })
        .partition(|entry| is_note(&entry.path));
    let groups = if matches!(trigger.kind, TriggerKind::WikiLink { embed: true, .. }) {
        vec![attachments, notes]
    } else {
        vec![notes]
    };
    let mut items = Vec::new();
    for group in groups.iter().filter(|group| !group.is_empty()) {
        for entry in filter_files(group, &trigger.query, LINK_LIMIT) {
            let name = link_name(&entry.name).to_owned();
            let icon = if has_extension_in(&entry.path, &IMAGE_EXTENSIONS) {
                IconName::Image
            } else {
                IconName::FileText
            };
            let folder = entry
                .path
                .parent()
                .map(|parent| parent.as_str().to_owned())
                .unwrap_or_default();
            let completion = link_completion(trigger, &name);
            items.push(
                CompletionItem::new(icon, name, &trigger.query, completion).described(folder),
            );
        }
    }
    items
}

fn rank(names: &[String], query: &str) -> Vec<String> {
    if query.is_empty() {
        return names.to_vec();
    }
    let needle = query.to_lowercase();
    let mut scored: Vec<(f32, &String)> = names
        .iter()
        .filter_map(|name| fuzzy_match(&needle, name).map(|found| (found.score, name)))
        .collect();
    scored.sort_by(|left, right| right.0.total_cmp(&left.0));
    scored.into_iter().map(|(_, name)| name.clone()).collect()
}

#[must_use]
pub(crate) fn heading_items(headings: &[String], trigger: &Trigger) -> Vec<CompletionItem> {
    rank(headings, &trigger.query)
        .into_iter()
        .map(|heading| {
            let completion = link_completion(trigger, &heading);
            CompletionItem::new(IconName::Heading, heading, &trigger.query, completion)
        })
        .collect()
}

#[must_use]
pub(crate) fn tag_items(tags: &[String], trigger: &Trigger) -> Vec<CompletionItem> {
    rank(tags, &trigger.query)
        .into_iter()
        .take(TAG_LIMIT)
        .map(|tag| {
            let completion = Completion::Insert {
                replace: trigger.span,
                cursor_after: tag.len(),
                text: tag.clone(),
            };
            CompletionItem::new(
                IconName::Hash,
                format!("#{tag}"),
                &trigger.query,
                completion,
            )
        })
        .collect()
}

#[must_use]
pub(crate) fn find_note<'files>(
    files: &'files [RelativePath],
    target: &str,
) -> Option<&'files RelativePath> {
    if target.is_empty() {
        return None;
    }
    let name = if link_name(target).len() < target.len() {
        target.to_owned()
    } else {
        format!("{target}.md")
    };
    let by_name = |wanted: &str| {
        files
            .iter()
            .find(|path| path.file_name().eq_ignore_ascii_case(wanted))
    };
    by_name(&name)
        .or_else(|| by_name(target))
        .or_else(|| files.iter().find(|path| path.as_str() == name))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::trigger::TriggerKind;

    fn paths(raw: &[&str]) -> Vec<RelativePath> {
        raw.iter()
            .map(|path| RelativePath::new(path).expect("valid path"))
            .collect()
    }

    fn trigger(kind: TriggerKind, start: usize, query: &str) -> Trigger {
        Trigger {
            kind,
            span: Span::new(start, start + query.len()),
            query: query.to_owned(),
        }
    }

    fn inserted(item: &CompletionItem) -> (Span, &str, usize) {
        match item.completion() {
            Completion::Insert {
                replace,
                text,
                cursor_after,
            } => (*replace, text.as_str(), *cursor_after),
            Completion::Slash { .. } => panic!("expected an insert completion"),
        }
    }

    const LINK: TriggerKind = TriggerKind::WikiLink {
        embed: false,
        closed: false,
    };
    const CLOSED_LINK: TriggerKind = TriggerKind::WikiLink {
        embed: false,
        closed: true,
    };
    const EMBED: TriggerKind = TriggerKind::WikiLink {
        embed: true,
        closed: false,
    };

    fn wiki(kind: TriggerKind, query: &str) -> Trigger {
        trigger(kind, 2, query)
    }

    fn labels(items: &[CompletionItem]) -> Vec<&str> {
        items.iter().map(CompletionItem::label).collect()
    }

    #[test]
    fn link_completion_closes_the_link_once() {
        let files = paths(&["Projects/Roadmap.md", "Ideas.md", "pic.png"]);
        let items = link_items(&files, &wiki(LINK, "road"));
        assert_eq!(labels(&items), ["Roadmap"]);
        assert_eq!(items[0].description(), "Projects");
        assert_eq!(inserted(&items[0]), (Span::new(2, 6), "Roadmap]]", 9));
        let items = link_items(&files, &wiki(CLOSED_LINK, "road"));
        assert_eq!(inserted(&items[0]), (Span::new(2, 8), "Roadmap]]", 9));
    }

    #[test]
    fn links_list_notes_only_and_embeds_list_attachments_first() {
        let files = paths(&["b.md", "board.canvas", "a.png"]);
        let links = link_items(&files, &wiki(LINK, ""));
        assert_eq!(labels(&links), ["b", "board"]);
        let embeds = link_items(&files, &wiki(EMBED, ""));
        assert_eq!(labels(&embeds), ["a.png", "b", "board"]);
        assert_eq!(embeds[0].icon(), IconName::Image);
    }

    #[test]
    fn heading_completion_inserts_heading_and_closes() {
        let headings = vec![
            "Plan".to_owned(),
            "Goals".to_owned(),
            "Planning notes".to_owned(),
        ];
        let heading = trigger(
            TriggerKind::Heading {
                note: "Roadmap".to_owned(),
                closed: false,
            },
            10,
            "pla",
        );
        let items = heading_items(&headings, &heading);
        assert_eq!(labels(&items), ["Plan", "Planning notes"]);
        assert_eq!(inserted(&items[0]), (Span::new(10, 13), "Plan]]", 6));
        assert_eq!(items[0].highlights(), [0..1, 1..2, 2..3]);
    }

    #[test]
    fn tag_completion_replaces_the_query_with_the_tag() {
        let tags = vec![
            "project".to_owned(),
            "personal".to_owned(),
            "work".to_owned(),
        ];
        let tag = trigger(TriggerKind::Tag, 1, "proj");
        let items = tag_items(&tags, &tag);
        assert_eq!(items.len(), 1);
        assert_eq!(items[0].label(), "#project");
        assert_eq!(inserted(&items[0]), (Span::new(1, 5), "project", 7));
        let everything = trigger(TriggerKind::Tag, 1, "");
        assert_eq!(tag_items(&tags, &everything).len(), 3);
    }

    #[test]
    fn notes_resolve_by_name_then_path() {
        let files = paths(&["a/Roadmap.md", "b/board.canvas", "c/Plain"]);
        assert_eq!(
            find_note(&files, "roadmap").map(RelativePath::as_str),
            Some("a/Roadmap.md")
        );
        assert_eq!(
            find_note(&files, "board.canvas").map(RelativePath::as_str),
            Some("b/board.canvas")
        );
        assert_eq!(
            find_note(&files, "Plain").map(RelativePath::as_str),
            Some("c/Plain")
        );
        assert_eq!(find_note(&files, "missing"), None);
        assert_eq!(find_note(&files, ""), None);
    }
}
