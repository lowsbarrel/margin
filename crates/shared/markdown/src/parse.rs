use crate::block::{Block, BlockKind, HeadingStyle};
use crate::inline_parse::Ctx;
use crate::link_ref::{LinkDefinitions, normalize_label, parse_definition};
use crate::span::Span;
use crate::text::{Line, is_blank, leading_spaces};
use crate::{parse_container, parse_html, parse_leaf, parse_list, table};

pub(crate) struct Parsed {
    pub blocks: Vec<Block>,
    pub definitions: LinkDefinitions,
}

pub(crate) fn parse(source: &str, lines: &[Line], at_start: bool) -> Parsed {
    let mut definitions = LinkDefinitions::new();
    let mut blocks = parse_blocks(source, lines, at_start, &mut definitions);
    let ctx = Ctx::new(source, &definitions);
    fill(&ctx, &mut blocks);
    Parsed {
        blocks,
        definitions,
    }
}

pub(crate) fn parse_blocks(
    source: &str,
    lines: &[Line],
    at_start: bool,
    definitions: &mut LinkDefinitions,
) -> Vec<Block> {
    let mut blocks: Vec<Block> = Vec::new();
    let mut index = 0;
    while index < lines.len() {
        let Some(line) = lines.get(index) else {
            break;
        };
        if is_blank(line.text(source)) {
            index += 1;
            continue;
        }
        let hit = last_block(source, lines, index, at_start, definitions)
            .or_else(|| paragraph(source, lines, index));
        match hit {
            Some((block, next)) => {
                blocks.push(block);
                index = next.max(index + 1);
            }
            None => index += 1,
        }
    }
    blocks
}

fn last_block(
    source: &str,
    lines: &[Line],
    index: usize,
    at_start: bool,
    definitions: &mut LinkDefinitions,
) -> Option<(Block, usize)> {
    let line = lines.get(index)?;
    let text = line.text(source);
    if is_blank(text) {
        return None;
    }
    if at_start
        && index == 0
        && let Some(hit) = parse_leaf::frontmatter_block(source, lines)
    {
        return Some(hit);
    }
    if let Some(hit) = parse_leaf::thematic_break(source, lines, index) {
        return Some(hit);
    }
    if let Some(hit) = parse_leaf::atx_heading(source, lines, index) {
        return Some(hit);
    }
    if let Some(hit) = parse_leaf::fence(source, lines, index) {
        return Some(hit);
    }
    if let Some(hit) = parse_leaf::block_math(source, lines, index) {
        return Some(hit);
    }
    if let Some(hit) = parse_leaf::comment_block(source, lines, index) {
        return Some(hit);
    }
    if let Some(hit) = parse_container::colon_callout(source, lines, index, definitions) {
        return Some(hit);
    }
    if let Some(hit) = parse_container::blockquote(source, lines, index, definitions) {
        return Some(hit);
    }
    if let Some(hit) = parse_list::list(source, lines, index, definitions) {
        return Some(hit);
    }
    if let Some(hit) = table(source, lines, index) {
        return Some(hit);
    }
    if let Some(hit) = parse_list::footnote_definition(source, lines, index, definitions) {
        return Some(hit);
    }
    if let Some(hit) = link_reference(source, lines, index, definitions) {
        return Some(hit);
    }
    if let Some(hit) = parse_html::indented_code(source, lines, index) {
        return Some(hit);
    }
    if let Some(hit) = parse_html::html_block(source, lines, index) {
        return Some(hit);
    }
    None
}

#[must_use]
pub(crate) fn starts_block(text: &str) -> bool {
    parse_leaf::is_thematic_break(text)
        || parse_leaf::is_atx_heading(text)
        || parse_leaf::is_fence(text)
        || parse_leaf::is_block_math(text)
        || parse_leaf::is_comment_block(text)
        || parse_html::is_html_block(text)
        || is_link_reference(text)
        || parse_container::is_colon_callout(text)
        || parse_container::is_blockquote(text)
        || parse_list::is_list_item(text)
        || parse_list::is_footnote_definition(text)
}

fn setext_level(text: &str) -> Option<u8> {
    let indent = leading_spaces(text);
    if indent > 3 {
        return None;
    }
    let rest = text.get(indent..)?.trim_end_matches([' ', '\t']);
    let ch = rest.chars().next()?;
    if ch != '=' && ch != '-' {
        return None;
    }
    if !rest.chars().all(|value| value == ch) {
        return None;
    }
    Some(if ch == '=' { 1 } else { 2 })
}

fn paragraph(source: &str, lines: &[Line], index: usize) -> Option<(Block, usize)> {
    let first = lines.get(index)?;
    let mut at = index;
    let mut last_end = first.end;
    while let Some(line) = lines.get(at) {
        let text = line.text(source);
        if at > index {
            if is_blank(text) {
                break;
            }
            if let Some(level) = setext_level(text) {
                let block = Block::new(
                    BlockKind::Heading {
                        level,
                        style: HeadingStyle::Setext,
                    },
                    Span::new(first.start, line.end),
                )
                .with_content(Span::new(first.start, last_end))
                .with_markers(vec![line.span()]);
                return Some((block, at + 1));
            }
            if starts_block(text) {
                break;
            }
        }
        last_end = line.end;
        at += 1;
    }
    Some((
        Block::new(BlockKind::Paragraph, Span::new(first.start, last_end)),
        at,
    ))
}

fn fill(ctx: &Ctx<'_>, blocks: &mut [Block]) {
    for block in blocks.iter_mut() {
        match &mut block.kind {
            BlockKind::Paragraph | BlockKind::Heading { .. } => {
                block.inlines = ctx.parse(block.content);
            }
            BlockKind::Table(table) => {
                for row in &mut table.rows {
                    for cell in &mut row.cells {
                        cell.inlines = ctx.parse(cell.span);
                    }
                }
            }
            _ => {}
        }
        fill(ctx, &mut block.children);
    }
}

fn heading_above(text: &str) -> bool {
    if parse_leaf::is_atx_heading(text) {
        return true;
    }
    let trimmed = text.trim_end_matches([' ', '\t']);
    trimmed.len() > 1
        && (trimmed.chars().all(|ch| ch == '=') || trimmed.chars().all(|ch| ch == '-'))
}

pub(crate) fn table(source: &str, lines: &[Line], index: usize) -> Option<(Block, usize)> {
    let anchored = index == 0
        || lines.get(index - 1).is_some_and(|line| {
            let text = line.text(source);
            is_blank(text) || heading_above(text)
        });
    let (model, next) = table::parse(source, lines, index, anchored)?;
    let span = model.span;
    Some((Block::new(BlockKind::Table(model), span), next))
}

#[must_use]
pub(crate) fn is_link_reference(text: &str) -> bool {
    parse_definition(text).is_some()
}

pub(crate) fn link_reference(
    source: &str,
    lines: &[Line],
    index: usize,
    definitions: &mut LinkDefinitions,
) -> Option<(Block, usize)> {
    let line = lines.get(index)?;
    let (label, target) = parse_definition(line.text(source))?;
    definitions.insert(label, target);
    let block = Block::new(
        BlockKind::LinkReference {
            label: normalize_label(label),
        },
        line.span(),
    );
    Some((block, index + 1))
}
