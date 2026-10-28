use std::rc::Rc;

use margin_kernel::RelativePath;
use margin_workbench::VaultServices;

use crate::diff_view::PreviewDiff;
use crate::line_diff::LineDiff;

#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) enum Comparison {
    Ready(LineDiff),
    TooLarge,
}

impl Comparison {
    pub(crate) fn into_preview(self) -> PreviewDiff {
        match self {
            Self::Ready(diff) => PreviewDiff::Ready(Rc::new(diff)),
            Self::TooLarge => PreviewDiff::TooLarge,
        }
    }
}

pub(crate) fn compare_with_current(
    services: &VaultServices,
    note: &RelativePath,
    snapshot: &str,
) -> Result<Comparison, margin_history::Error> {
    let earlier = services.history.read(note, snapshot)?;
    let current = services.files.read_file(note).unwrap_or_default();
    let diff = LineDiff::between(
        &String::from_utf8_lossy(&earlier),
        &String::from_utf8_lossy(&current),
    );
    Ok(diff.map_or(Comparison::TooLarge, Comparison::Ready))
}

#[must_use]
pub(crate) fn restore_snapshot(
    services: &VaultServices,
    note: &RelativePath,
    snapshot: &str,
) -> bool {
    if let Ok(current) = services.files.read_file(note)
        && services.history.save(note, &current).is_err()
    {
        return false;
    }
    services
        .history
        .read(note, snapshot)
        .is_ok_and(|bytes| services.files.write_file(note, &bytes).is_ok())
}
