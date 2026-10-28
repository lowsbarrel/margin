use crate::layout::Tab;

#[must_use]
pub fn clamp_insert_index<T>(tabs: &[Tab<T>], pinned: bool, index: usize) -> usize {
    let pinned_count = tabs.iter().filter(|tab| tab.pinned).count();
    if pinned {
        index.min(pinned_count)
    } else {
        index.clamp(pinned_count, tabs.len().max(pinned_count))
    }
}

#[must_use]
pub fn reorder_tab_gap<T>(tabs: &[Tab<T>], from: usize, gap: usize) -> usize {
    let Some(tab) = tabs.get(from) else {
        return gap;
    };
    let pinned_rest = tabs
        .iter()
        .enumerate()
        .filter(|(index, other)| *index != from && other.pinned)
        .count();
    let rest_len = tabs.len() - 1;
    let wanted = if gap > from { gap - 1 } else { gap };
    let to = if tab.pinned {
        wanted.min(pinned_rest)
    } else {
        wanted.clamp(pinned_rest, rest_len.max(pinned_rest))
    };
    if to <= from { to } else { to + 1 }
}
