use margin_markdown::{Document, Span, TextEdit};
use margin_text_buffer::{Change, Selection};

use crate::fuzzy::{self, FuzzyMatch};
use crate::mapping::{self, Assoc};
use crate::prefix;
use crate::text::{self, Line};

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum SlashCommand {
    Heading1,
    Heading2,
    Heading3,
    BulletList,
    NumberedList,
    TodoList,
    Quote,
    CodeBlock,
    Table,
    Callout,
    MathBlock,
    Mermaid,
    Divider,
    Link,
    EmbedNote,
    Date,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct SlashItem {
    pub command: SlashCommand,
    pub label_key: &'static str,
    pub description_key: &'static str,
    pub keywords: &'static [&'static str],
}

const fn item(
    command: SlashCommand,
    label_key: &'static str,
    description_key: &'static str,
    keywords: &'static [&'static str],
) -> SlashItem {
    SlashItem {
        command,
        label_key,
        description_key,
        keywords,
    }
}

pub const SLASH_ITEMS: [SlashItem; 16] = [
    item(
        SlashCommand::Heading1,
        "slash_heading1",
        "slash_heading1_description",
        &["title", "big", "large", "h1"],
    ),
    item(
        SlashCommand::Heading2,
        "slash_heading2",
        "slash_heading2_description",
        &["subtitle", "medium", "h2"],
    ),
    item(
        SlashCommand::Heading3,
        "slash_heading3",
        "slash_heading3_description",
        &["subtitle", "small", "h3"],
    ),
    item(
        SlashCommand::BulletList,
        "slash_bullet",
        "slash_bullet_description",
        &["unordered", "point", "list", "ul"],
    ),
    item(
        SlashCommand::NumberedList,
        "slash_numbered",
        "slash_numbered_description",
        &["numbered", "ordered", "list", "ol"],
    ),
    item(
        SlashCommand::TodoList,
        "slash_todo",
        "slash_todo_description",
        &["todo", "task", "list", "check", "checkbox"],
    ),
    item(
        SlashCommand::Quote,
        "slash_quote",
        "slash_quote_description",
        &["blockquote", "quotes"],
    ),
    item(
        SlashCommand::CodeBlock,
        "slash_code",
        "slash_code_description",
        &["codeblock", "code", "snippet", "fence"],
    ),
    item(
        SlashCommand::Table,
        "slash_table",
        "slash_table_description",
        &["table", "rows", "columns", "grid"],
    ),
    item(
        SlashCommand::Callout,
        "slash_callout",
        "slash_callout_description",
        &["callout", "admonition", "alert", "info", "warning", "note"],
    ),
    item(
        SlashCommand::MathBlock,
        "slash_math_block",
        "slash_math_block_description",
        &["math", "equation", "formula", "latex", "katex", "block"],
    ),
    item(
        SlashCommand::Mermaid,
        "slash_mermaid",
        "slash_mermaid_description",
        &[
            "mermaid",
            "diagram",
            "flowchart",
            "graph",
            "sequence",
            "chart",
        ],
    ),
    item(
        SlashCommand::Divider,
        "slash_divider",
        "slash_divider_description",
        &["horizontal rule", "hr", "divider", "separator"],
    ),
    item(
        SlashCommand::Link,
        "slash_link",
        "slash_link_description",
        &["link", "url", "href"],
    ),
    item(
        SlashCommand::EmbedNote,
        "slash_embed_note",
        "slash_embed_note_description",
        &["embed", "transclude", "include", "note", "reference"],
    ),
    item(
        SlashCommand::Date,
        "slash_date",
        "slash_date_description",
        &["date", "today", "time"],
    ),
];

const TABLE: &str = "|   |   |   |\n| --- | --- | --- |\n|   |   |   |";

fn item_score(item: &SlashItem, needle: &str, title: &str) -> Option<f32> {
    let mut best: Option<FuzzyMatch> = fuzzy::fuzzy_match(needle, title);
    for term in item.keywords {
        let Some(found) = fuzzy::fuzzy_match(needle, term) else {
            continue;
        };
        if best
            .as_ref()
            .is_none_or(|current| found.score > current.score)
        {
            best = Some(found);
        }
    }
    best.map(|found| found.score)
}

#[must_use]
pub fn slash_items<Label: AsRef<str>>(
    query: &str,
    label: impl Fn(&'static str) -> Label,
) -> Vec<&'static SlashItem> {
    let needle = query.trim().to_lowercase();
    if needle.is_empty() {
        return SLASH_ITEMS.iter().collect();
    }
    let mut scored: Vec<(&'static SlashItem, f32)> = SLASH_ITEMS
        .iter()
        .filter_map(|item| {
            let title = label(item.label_key);
            item_score(item, &needle, title.as_ref()).map(|score| (item, score))
        })
        .collect();
    scored.sort_by(|left, right| right.1.total_cmp(&left.1));
    scored.into_iter().map(|(item, _)| item).collect()
}

struct SlashEdit {
    edit: TextEdit,
    caret: usize,
}

fn marker_edit(document: &Document, trigger: Span, marker: &str) -> SlashEdit {
    let source = document.text();
    let line = Line::at(document, trigger.start);
    let before = text::slice(source, line.start, trigger.start);
    let indent = text::leading_space(before);
    let head = before.get(indent.len()..).unwrap_or_default();
    let tail = text::slice(source, trigger.end, line.end);
    let joined = if head.is_empty() {
        tail.trim_start().to_owned()
    } else {
        format!("{} {}", head.trim_end(), tail.trim_start())
    };
    let body = joined
        .get(prefix::block_prefix(&joined)..)
        .unwrap_or_default()
        .trim_end();
    SlashEdit {
        edit: TextEdit::new(line.span(), format!("{indent}{marker}{body}")),
        caret: line.start + indent.len() + marker.len(),
    }
}

fn block_edit(document: &Document, trigger: Span, block: &str, caret: usize) -> SlashEdit {
    let source = document.text();
    let line = Line::at(document, trigger.start);
    let before = text::slice(source, line.start, trigger.start);
    let head = before
        .get(prefix::block_prefix(before)..)
        .unwrap_or_default()
        .trim();
    let tail = text::slice(source, trigger.end, line.end).trim();
    let prefix = if head.is_empty() {
        String::new()
    } else {
        format!("{head}\n")
    };
    let suffix = if tail.is_empty() {
        String::new()
    } else {
        format!("\n{tail}")
    };
    SlashEdit {
        caret: line.start + prefix.len() + caret,
        edit: TextEdit::new(line.span(), format!("{prefix}{block}{suffix}")),
    }
}

fn inline_edit(trigger: Span, text: &str, caret: usize) -> SlashEdit {
    SlashEdit {
        edit: TextEdit::new(trigger, text),
        caret: trigger.start + caret,
    }
}

fn slash_edit(document: &Document, trigger: Span, command: SlashCommand, today: &str) -> SlashEdit {
    match command {
        SlashCommand::Heading1 => marker_edit(document, trigger, "# "),
        SlashCommand::Heading2 => marker_edit(document, trigger, "## "),
        SlashCommand::Heading3 => marker_edit(document, trigger, "### "),
        SlashCommand::BulletList => marker_edit(document, trigger, "- "),
        SlashCommand::NumberedList => marker_edit(document, trigger, "1. "),
        SlashCommand::TodoList => marker_edit(document, trigger, "- [ ] "),
        SlashCommand::Quote => marker_edit(document, trigger, "> "),
        SlashCommand::CodeBlock => block_edit(document, trigger, "```\n\n```", 4),
        SlashCommand::Table => block_edit(document, trigger, TABLE, 2),
        SlashCommand::Callout => block_edit(document, trigger, "> [!note]\n> ", 12),
        SlashCommand::MathBlock => block_edit(document, trigger, "$$\n\n$$", 3),
        SlashCommand::Mermaid => block_edit(document, trigger, "```mermaid\n\n```", 11),
        SlashCommand::Divider => block_edit(document, trigger, "---", 3),
        SlashCommand::Link => inline_edit(trigger, "[]()", 1),
        SlashCommand::EmbedNote => inline_edit(trigger, "![[]]", 3),
        SlashCommand::Date => inline_edit(trigger, today, today.len()),
    }
}

#[must_use]
pub fn apply_slash(
    document: &Document,
    selections: &[Selection],
    trigger: Span,
    command: SlashCommand,
    today: &str,
) -> Option<Change> {
    let length = document.text().len();
    if trigger.start > trigger.end || trigger.end > length {
        return None;
    }
    let SlashEdit { edit, caret } = slash_edit(document, trigger, command, today);
    let replaced = edit.span;
    let edits = [edit];
    let mut moved: Vec<Selection> = selections
        .iter()
        .filter(|selection| !selection.span().overlaps(replaced.start, replaced.end))
        .map(|selection| mapping::map_selection(&edits, *selection, Assoc::Before))
        .collect();
    moved.push(Selection::cursor(caret));
    moved.sort_by_key(Selection::start);
    let [edit] = edits;
    Some(Change::new(vec![edit]).with_selections(moved))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::testing::{apply, cursor_text};

    fn slash(source: &str, command: SlashCommand) -> String {
        let at = source.find('/').expect("the source holds a slash trigger");
        let end = source[at..].find(' ').map_or(source.len(), |gap| at + gap);
        let document = Document::parse(source);
        let trigger = Span::new(at, end);
        let selections = [Selection::cursor(end)];
        let change = apply_slash(&document, &selections, trigger, command, "28 October 2026")
            .expect("the trigger lies inside the note");
        let text = apply(&document, &change);
        cursor_text(&text, &change.selections.unwrap_or_default())
    }

    #[test]
    fn turns_the_line_into_a_block_marker() {
        assert_eq!(slash("/h1", SlashCommand::Heading1), "# |");
        assert_eq!(slash("- item /todo", SlashCommand::TodoList), "- [ ] |item");
        assert_eq!(slash("  /num", SlashCommand::NumberedList), "  1. |");
    }

    #[test]
    fn inserts_block_templates_around_the_line() {
        assert_eq!(
            slash("text /code", SlashCommand::CodeBlock),
            "text\n```\n|\n```"
        );
        assert_eq!(
            slash("/table", SlashCommand::Table),
            "| |  |   |   |\n| --- | --- | --- |\n|   |   |   |"
        );
        assert_eq!(slash("/call", SlashCommand::Callout), "> [!note]\n> |");
        assert_eq!(slash("/hr", SlashCommand::Divider), "---|");
    }

    #[test]
    fn inserts_inline_snippets() {
        assert_eq!(slash("see /link", SlashCommand::Link), "see [|]()");
        assert_eq!(slash("/embed", SlashCommand::EmbedNote), "![[|]]");
        assert_eq!(slash("on /date", SlashCommand::Date), "on 28 October 2026|");
    }

    #[test]
    fn ranks_items_by_fuzzy_score() {
        let label = |key: &'static str| key.trim_start_matches("slash_").replace('_', " ");
        let all = slash_items("", label);
        assert_eq!(all.len(), SLASH_ITEMS.len());
        let found = slash_items("h2", label);
        assert_eq!(
            found.first().map(|item| item.command),
            Some(SlashCommand::Heading2)
        );
        let found = slash_items("diagram", label);
        assert_eq!(
            found.first().map(|item| item.command),
            Some(SlashCommand::Mermaid)
        );
        assert!(slash_items("zzzz", label).is_empty());
    }
}
