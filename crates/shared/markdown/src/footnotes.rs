use std::collections::BTreeMap;

use crate::block::{Block, BlockKind};
use crate::inline::{Inline, InlineKind};
use crate::span::Span;

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct FootnoteDefinition {
    pub id: String,
    pub span: Span,
    pub body: Span,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct FootnoteReference {
    pub id: String,
    pub span: Span,
}

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct FootnoteIndex {
    pub definitions: Vec<FootnoteDefinition>,
    pub references: Vec<FootnoteReference>,
    pub numbers: BTreeMap<String, usize>,
}

impl FootnoteIndex {
    #[must_use]
    pub fn number(&self, id: &str) -> Option<usize> {
        self.numbers.get(id).copied()
    }

    #[must_use]
    pub fn definition(&self, id: &str) -> Option<&FootnoteDefinition> {
        self.definitions.iter().find(|entry| entry.id == id)
    }

    #[must_use]
    pub fn references_to(&self, id: &str) -> Vec<&FootnoteReference> {
        self.references
            .iter()
            .filter(|entry| entry.id == id)
            .collect()
    }
}

pub(crate) fn build(blocks: &[Block]) -> FootnoteIndex {
    let mut index = FootnoteIndex::default();
    for block in blocks {
        definitions_of(block, &mut index);
    }
    for block in blocks {
        references_of(block, &mut index);
    }
    let ids: Vec<String> = index
        .references
        .iter()
        .map(|reference| reference.id.clone())
        .collect();
    for id in ids {
        if !index.numbers.contains_key(&id) {
            let next = index.numbers.len() + 1;
            index.numbers.insert(id, next);
        }
    }
    index
}

fn definitions_of(block: &Block, index: &mut FootnoteIndex) {
    if let BlockKind::FootnoteDefinition { id } = &block.kind
        && !index.definitions.iter().any(|entry| entry.id == *id)
    {
        index.definitions.push(FootnoteDefinition {
            id: id.clone(),
            span: block.span,
            body: block.content,
        });
    }
    for child in &block.children {
        definitions_of(child, index);
    }
}

fn references_of(block: &Block, index: &mut FootnoteIndex) {
    for inline in &block.inlines {
        collect(inline, index);
    }
    if let Some(table) = block.table() {
        for row in &table.rows {
            for cell in &row.cells {
                for inline in &cell.inlines {
                    collect(inline, index);
                }
            }
        }
    }
    for child in &block.children {
        references_of(child, index);
    }
}

fn collect(inline: &Inline, index: &mut FootnoteIndex) {
    if let InlineKind::FootnoteReference { id } = &inline.kind
        && index.definitions.iter().any(|entry| entry.id == *id)
    {
        index.references.push(FootnoteReference {
            id: id.clone(),
            span: inline.span,
        });
    }
    for child in &inline.children {
        collect(child, index);
    }
}
