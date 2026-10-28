use std::rc::Rc;

use gpui_kit::assets::IconName;
use gpui_kit::component::button::{Button, ButtonVariants};
use gpui_kit::component::menu::{PopupMenu, PopupMenuItem};
use gpui_kit::component::{Icon, Sizable};
use gpui_kit::{App, Window};
use margin_i18n::t;
use margin_markdown::Document;

use crate::locate::{CellLocation, table_at};
use crate::ops::TableOp;

pub type OnTableOp = Rc<dyn Fn(TableOp, &mut Window, &mut App)>;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum TableMenu {
    Row,
    Column,
    Cell,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum TableControl {
    AddRow,
    AddColumn,
}

struct Entry {
    key: &'static str,
    op: TableOp,
    enabled: bool,
}

const fn entry(key: &'static str, op: TableOp, enabled: bool) -> Entry {
    Entry { key, op, enabled }
}

fn groups(scope: TableMenu, rows: usize, cols: usize, at: &CellLocation) -> Vec<Vec<Entry>> {
    let (row, col) = (at.row, at.column);
    let insert_rows = vec![
        entry(
            "editor_table_insert_row_above",
            TableOp::InsertRowAbove,
            true,
        ),
        entry(
            "editor_table_insert_row_below",
            TableOp::InsertRowBelow,
            true,
        ),
    ];
    let insert_columns = vec![
        entry(
            "editor_table_insert_column_left",
            TableOp::InsertColumnLeft,
            true,
        ),
        entry(
            "editor_table_insert_column_right",
            TableOp::InsertColumnRight,
            true,
        ),
    ];
    let move_rows = vec![
        entry("editor_table_move_row_up", TableOp::MoveRowUp, row > 0),
        entry(
            "editor_table_move_row_down",
            TableOp::MoveRowDown,
            row + 1 < rows,
        ),
    ];
    let move_columns = vec![
        entry(
            "editor_table_move_column_left",
            TableOp::MoveColumnLeft,
            col > 0,
        ),
        entry(
            "editor_table_move_column_right",
            TableOp::MoveColumnRight,
            col + 1 < cols,
        ),
    ];
    let sort = vec![
        entry("editor_table_sort_ascending", TableOp::SortAscending, true),
        entry(
            "editor_table_sort_descending",
            TableOp::SortDescending,
            true,
        ),
    ];
    let align = vec![
        entry("editor_table_align_left", TableOp::AlignLeft, true),
        entry("editor_table_align_center", TableOp::AlignCenter, true),
        entry("editor_table_align_right", TableOp::AlignRight, true),
    ];
    let delete_row = entry("editor_table_delete_row", TableOp::DeleteRow, rows > 1);
    let delete_column = entry(
        "editor_table_delete_column",
        TableOp::DeleteColumn,
        cols > 1,
    );
    match scope {
        TableMenu::Row => vec![insert_rows, move_rows, vec![delete_row]],
        TableMenu::Column => vec![
            insert_columns,
            move_columns,
            sort,
            align,
            vec![delete_column],
        ],
        TableMenu::Cell => vec![
            insert_rows,
            insert_columns,
            sort,
            align,
            vec![delete_row, delete_column],
        ],
    }
}

#[must_use]
pub fn table_menu(
    menu: PopupMenu,
    document: &Document,
    at: &CellLocation,
    scope: TableMenu,
    on_op: &OnTableOp,
    cx: &App,
) -> PopupMenu {
    let Some(table) = table_at(document, at.table.start) else {
        return menu;
    };
    let mut menu = menu;
    for (index, group) in groups(scope, table.rows.len(), table.cols, at)
        .into_iter()
        .enumerate()
    {
        if index > 0 {
            menu = menu.separator();
        }
        for item in group {
            let on_op = Rc::clone(on_op);
            let op = item.op;
            menu = menu.item(
                PopupMenuItem::new(t(cx, item.key))
                    .disabled(!item.enabled)
                    .on_click(move |_, window, cx| on_op(op, window, cx)),
            );
        }
    }
    menu
}

#[must_use]
pub fn table_control(control: TableControl, on_op: OnTableOp, cx: &App) -> Button {
    let (id, key, op) = match control {
        TableControl::AddRow => ("table-add-row", "editor_table_add_row", TableOp::AppendRow),
        TableControl::AddColumn => (
            "table-add-column",
            "editor_table_add_column",
            TableOp::AppendColumn,
        ),
    };
    Button::new(id)
        .ghost()
        .xsmall()
        .icon(Icon::new(IconName::Plus))
        .tooltip(t(cx, key))
        .on_click(move |_, window, cx| on_op(op, window, cx))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::locate::cell_at;

    #[test]
    fn destructive_items_come_last_and_respect_the_shape() {
        let document = Document::parse("| a | b |\n| - | - |\n");
        let at = cell_at(&document, 2).expect("cell");
        let cell = groups(TableMenu::Cell, 1, 2, &at);
        let last = cell.last().expect("group");
        assert_eq!(
            last.iter().map(|item| item.op).collect::<Vec<_>>(),
            [TableOp::DeleteRow, TableOp::DeleteColumn]
        );
        assert!(
            !last
                .iter()
                .any(|item| item.op == TableOp::DeleteRow && item.enabled)
        );
        let row = groups(TableMenu::Row, 1, 2, &at);
        assert!(
            row.iter()
                .flatten()
                .all(|item| item.op != TableOp::MoveRowUp || !item.enabled)
        );
    }
}
