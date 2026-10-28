use gpui_kit::component::StyledExt;
use gpui_kit::prelude::FluentBuilder;
use gpui_kit::{AnyElement, App, Context, IntoElement, ParentElement, Styled, div};
use margin_dates::{CivilDate, UtcOffset};
use margin_history::Snapshot;
use margin_kernel::UnixMillis;
use margin_theme::{MarginTheme, TextScale};
use margin_workbench::AppServices;

use crate::diff_view::PreviewDiff;
use crate::history_panel::HistoryPanel;
use crate::snapshot_row::{RowLook, snapshot_row, stamp};
use crate::when::{self, Row};

pub(crate) fn history_rows(
    snapshots: &[Snapshot],
    cx: &App,
) -> (Vec<Row<CivilDate>>, UnixMillis, UtcOffset) {
    let now = cx.global::<AppServices>().clock.now();
    let offset = when::local_offset(now);
    let dates = snapshots
        .iter()
        .map(|snapshot| when::local_date(stamp(snapshot), offset));
    (when::grouped_rows(dates), now, offset)
}

pub(crate) fn history_list(
    snapshots: &[Snapshot],
    preview: Option<(&str, &PreviewDiff)>,
    selected: Option<usize>,
    restoring: Option<usize>,
    focused: bool,
    cx: &Context<'_, HistoryPanel>,
) -> Vec<AnyElement> {
    let tertiary = MarginTheme::palette(cx).text_tertiary;
    let (rows, now, offset) = history_rows(snapshots, cx);
    let mut children = Vec::with_capacity(rows.len());
    for row in rows {
        let child = match row {
            Row::Group(date) => div()
                .when(!children.is_empty(), Styled::mt_3)
                .mb_1()
                .px_2()
                .py_1()
                .text_ui_xs()
                .font_semibold()
                .text_color(tertiary)
                .child(when::full_date_label(date, cx).to_uppercase())
                .into_any_element(),
            Row::Item(index) => {
                let snapshot = &snapshots[index];
                let diff = preview
                    .filter(|(filename, _)| *filename == snapshot.filename)
                    .map(|(_, diff)| diff);
                let look = match (diff.is_some(), selected == Some(index)) {
                    (true, _) => RowLook::Previewed,
                    (false, true) => RowLook::Selected { focused },
                    (false, false) => RowLook::Plain,
                };
                snapshot_row(
                    index,
                    snapshot,
                    when::snapshot_label(stamp(snapshot), now, offset, cx),
                    diff,
                    look,
                    restoring == Some(index),
                    cx,
                )
            }
        };
        children.push(child);
    }
    children
}
