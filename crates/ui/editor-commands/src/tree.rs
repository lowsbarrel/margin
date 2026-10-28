use margin_markdown::{Block, BlockKind, Document};

fn descend<'document>(blocks: &'document [Block], pos: usize, out: &mut Vec<&'document Block>) {
    for block in blocks {
        if block.span.contains(pos) {
            out.push(block);
            descend(&block.children, pos, out);
            return;
        }
    }
}

pub(crate) fn ancestors(document: &Document, pos: usize) -> Vec<&Block> {
    let mut out = Vec::new();
    descend(document.blocks(), pos, &mut out);
    out
}

pub(crate) fn fenced_code_at(document: &Document, pos: usize) -> Option<&Block> {
    ancestors(document, pos)
        .into_iter()
        .rev()
        .find(|block| matches!(block.kind, BlockKind::CodeBlock { fenced: true, .. }))
}

pub(crate) fn in_code_block(document: &Document, pos: usize) -> bool {
    ancestors(document, pos)
        .iter()
        .any(|block| block.is_code_block())
}

pub(crate) fn is_quote(kind: &BlockKind) -> bool {
    matches!(kind, BlockKind::BlockQuote | BlockKind::Callout { .. })
}

pub(crate) fn is_list(kind: &BlockKind) -> bool {
    matches!(kind, BlockKind::BulletList | BlockKind::OrderedList { .. })
}
