use margin_markdown::{BlockKind, Document, Inline, InlineKind, Span};

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum TriggerKind {
    Slash,
    WikiLink { embed: bool, closed: bool },
    Heading { note: String, closed: bool },
    Tag,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Trigger {
    pub kind: TriggerKind,
    pub span: Span,
    pub query: String,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Place {
    Literal,
    Table,
    Prose,
}

#[must_use]
pub fn completion_trigger(document: &Document, cursor: usize) -> Option<Trigger> {
    let text = document.text();
    if cursor > text.len() || !text.is_char_boundary(cursor) {
        return None;
    }
    let line = document.line_of(cursor);
    let start = document.line_span(line).start.min(cursor);
    let before = &text[start..cursor];
    match place(document, cursor) {
        Place::Literal => None,
        Place::Table => wiki(before, &text[cursor..], cursor)
            .or_else(|| tag(document.line_text(line), before, cursor)),
        Place::Prose => slash(before, cursor)
            .or_else(|| wiki(before, &text[cursor..], cursor))
            .or_else(|| tag(document.line_text(line), before, cursor)),
    }
}

fn place(document: &Document, cursor: usize) -> Place {
    let Some(block) = document.block_at(cursor) else {
        return Place::Prose;
    };
    match block.kind {
        BlockKind::CodeBlock { .. } | BlockKind::Frontmatter(_) => Place::Literal,
        BlockKind::Table(_) => Place::Table,
        _ if in_inline_code(&block.inlines, cursor) => Place::Literal,
        _ => Place::Prose,
    }
}

fn in_inline_code(inlines: &[Inline], cursor: usize) -> bool {
    inlines.iter().any(|inline| {
        (inline.kind == InlineKind::Code && inline.span.start < cursor && cursor < inline.span.end)
            || in_inline_code(&inline.children, cursor)
    })
}

fn slash(before: &str, cursor: usize) -> Option<Trigger> {
    let start = before
        .char_indices()
        .rev()
        .find(|(_, character)| character.is_whitespace())
        .map_or(0, |(index, character)| index + character.len_utf8());
    let run = &before[start..];
    let query = run.strip_prefix('/')?;
    Some(Trigger {
        kind: TriggerKind::Slash,
        span: Span::new(cursor - run.len(), cursor),
        query: query.to_owned(),
    })
}

fn wiki(before: &str, after: &str, cursor: usize) -> Option<Trigger> {
    let start = before.rfind(['[', ']', '|']).map_or(0, |index| index + 1);
    let head = before[..start].strip_suffix("[[")?;
    let embed = match head.strip_suffix('!') {
        Some(prefix) => !prefix.ends_with(['[', ']']),
        None if head.ends_with(['[', ']']) => return None,
        None => false,
    };
    let rest = &before[start..];
    let closed = after.starts_with("]]");
    let (kind, query) = match rest.split_once('#') {
        Some((note, heading)) => (
            TriggerKind::Heading {
                note: note.to_owned(),
                closed,
            },
            heading,
        ),
        None => (TriggerKind::WikiLink { embed, closed }, rest),
    };
    Some(Trigger {
        kind,
        span: Span::new(cursor - query.len(), cursor),
        query: query.to_owned(),
    })
}

const fn is_word(character: char) -> bool {
    character.is_ascii_alphanumeric() || character == '_'
}

const fn is_tag_character(character: char) -> bool {
    is_word(character) || character == '/' || character == '-'
}

fn is_heading_line(line: &str) -> bool {
    let level = line.bytes().take_while(|byte| *byte == b'#').count();
    (1..=6).contains(&level) && matches!(line.as_bytes().get(level), None | Some(b' ' | b'\t'))
}

fn tag(line: &str, before: &str, cursor: usize) -> Option<Trigger> {
    if is_heading_line(line) {
        return None;
    }
    let hash = before.rfind(|character| !is_tag_character(character))?;
    let query = before[hash..].strip_prefix('#')?;
    if !query.starts_with(|character: char| character.is_ascii_alphabetic()) {
        return None;
    }
    let previous = before[..hash].chars().next_back();
    if previous.is_some_and(|character| is_word(character) || character == '#' || character == '/')
    {
        return None;
    }
    Some(Trigger {
        kind: TriggerKind::Tag,
        span: Span::new(cursor - query.len(), cursor),
        query: query.to_owned(),
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn at_end(text: &str) -> Option<Trigger> {
        completion_trigger(&Document::parse(text), text.len())
    }

    fn at_caret(marked: &str) -> Option<Trigger> {
        let cursor = marked.find('|')?;
        let text = marked.replacen('|', "", 1);
        completion_trigger(&Document::parse(&text), cursor)
    }

    #[test]
    fn slash_opens_at_line_start_and_after_whitespace() {
        let trigger = at_end("/hea").expect("slash at line start");
        assert_eq!(trigger.kind, TriggerKind::Slash);
        assert_eq!(trigger.span, Span::new(0, 4));
        assert_eq!(trigger.query, "hea");
        let trigger = at_end("some text /").expect("slash after a space");
        assert_eq!(trigger.span, Span::new(10, 11));
        assert_eq!(trigger.query, "");
    }

    #[test]
    fn slash_needs_whitespace_before_it() {
        assert_eq!(at_end("and/or"), None);
        assert_eq!(at_end("/a b"), None);
        assert_eq!(at_end("http://x"), None);
    }

    #[test]
    fn nothing_triggers_inside_code_or_frontmatter() {
        assert_eq!(at_end("```\n/"), None);
        assert_eq!(at_caret("text `a /|b` more"), None);
        assert_eq!(at_caret("---\ntags: #ta|\n---\nbody"), None);
        assert_eq!(at_end("    indented /"), None);
    }

    #[test]
    fn wiki_link_reports_target_and_embed() {
        let trigger = at_end("see [[Proj").expect("wiki link");
        assert_eq!(
            trigger.kind,
            TriggerKind::WikiLink {
                embed: false,
                closed: false
            }
        );
        assert_eq!(trigger.span, Span::new(6, 10));
        assert_eq!(trigger.query, "Proj");
        let trigger = at_caret("![[pic|]]").expect("embed");
        assert_eq!(
            trigger.kind,
            TriggerKind::WikiLink {
                embed: true,
                closed: true
            }
        );
        assert_eq!(trigger.query, "pic");
    }

    #[test]
    fn wiki_link_rejects_extra_brackets_and_aliases() {
        assert_eq!(at_end("[[[x"), None);
        assert_eq!(at_end("[[note|ali"), None);
        assert_eq!(at_end("[[done]] more"), None);
    }

    #[test]
    fn heading_trigger_names_the_note() {
        let trigger = at_end("[[Roadmap#Pla").expect("heading");
        assert_eq!(
            trigger.kind,
            TriggerKind::Heading {
                note: "Roadmap".to_owned(),
                closed: false
            }
        );
        assert_eq!(trigger.span, Span::new(10, 13));
        assert_eq!(trigger.query, "Pla");
    }

    #[test]
    fn tag_needs_a_letter_and_a_boundary() {
        let trigger = at_end("idea #pro").expect("tag");
        assert_eq!(trigger.kind, TriggerKind::Tag);
        assert_eq!(trigger.span, Span::new(6, 9));
        assert_eq!(trigger.query, "pro");
        assert_eq!(
            at_end("#a/b-c").map(|trigger| trigger.query),
            Some("a/b-c".to_owned())
        );
        assert_eq!(at_end("#1"), None);
        assert_eq!(at_end("#"), None);
        assert_eq!(at_end("a#tag"), None);
        assert_eq!(at_end("##tag"), None);
    }

    #[test]
    fn tags_do_not_open_on_heading_lines() {
        assert_eq!(at_end("# Title #ta"), None);
        assert_eq!(
            at_end("#tag").map(|trigger| trigger.kind),
            Some(TriggerKind::Tag)
        );
    }

    #[test]
    fn tables_offer_links_and_tags_but_not_slash() {
        let table = "| a | b |\n| --- | --- |\n| x /";
        assert_eq!(at_end(table), None);
        let table = "| a | b |\n| --- | --- |\n| [[No";
        assert_eq!(
            at_end(table).map(|trigger| trigger.query),
            Some("No".to_owned())
        );
    }
}
