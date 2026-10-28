mod autopair;
mod block_drop;
mod block_type;
mod blocks;
mod editing;
mod fuzzy;
mod html_dom;
mod html_paste;
mod html_prepare;
mod indentation;
mod line_ops;
mod lines;
mod list_drop;
mod list_keys;
mod list_move;
mod list_tree;
mod mapping;
mod marks;
mod markup_context;
mod markup_delete;
mod newline;
mod paste;
mod prefix;
mod slash;
#[cfg(test)]
mod testing;
mod text;
mod tree;

pub use autopair::{autopair_input, delete_pair_backward};
pub use block_drop::{DragBlock, DragKind, Side, block_near, draggable_block, drop_beside};
pub use block_type::{BUBBLE_BLOCK_TYPES, BlockType, block_type, inside_code, set_block};
pub use blocks::{
    ListKind, exit_empty_quote, insert_link, set_link, toggle_heading, toggle_list, toggle_quote,
};
pub use editing::{Granularity, delete_backward, delete_forward, insert_text};
pub use fuzzy::{FuzzyMatch, fuzzy_match};
pub use html_paste::html_to_markdown;
pub use line_ops::delete_line;
pub use lines::{
    SelectionMode, delete_to_line_start, line_content_start, soft_break, toggle_checklist,
};
pub use list_drop::{DropGap, GapItem, drop_item, item_level, list_gaps};
pub use list_keys::{Direction, indent, move_list_item, outdent};
pub use list_tree::detect_indent_unit;
pub use marks::{MarkKind, active_marks, toggle_mark};
pub use markup_delete::delete_markup_backward;
pub use newline::{continue_markup, insert_newline};
pub use paste::{PasteMode, paste};
pub use slash::{SLASH_ITEMS, SlashCommand, SlashItem, apply_slash, slash_items};
