use crate::tab_order::{clamp_insert_index, reorder_tab_gap};

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct PaneId(u64);

impl PaneId {
    #[must_use]
    pub const fn value(self) -> u64 {
        self.0
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Side {
    Left,
    Right,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Tab<T> {
    pub item: T,
    pub pinned: bool,
}

impl<T> Tab<T> {
    const fn unpinned(item: T) -> Self {
        Self {
            item,
            pinned: false,
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Pane<T> {
    pub id: PaneId,
    pub tabs: Vec<Tab<T>>,
    pub active: Option<usize>,
}

impl<T> Pane<T> {
    #[must_use]
    pub fn active_tab(&self) -> Option<&Tab<T>> {
        self.tabs.get(self.active?)
    }

    fn detach(&mut self, index: usize) -> Option<Tab<T>> {
        if index >= self.tabs.len() {
            return None;
        }
        let tab = self.tabs.remove(index);
        self.active = match self.active {
            _ if self.tabs.is_empty() => None,
            Some(active) if active == index => Some(index.min(self.tabs.len() - 1)),
            Some(active) if index < active => Some(active - 1),
            other => other,
        };
        Some(tab)
    }

    fn insert(&mut self, tab: Tab<T>, at: Option<usize>) -> usize {
        let at = clamp_insert_index(&self.tabs, tab.pinned, at.unwrap_or(self.tabs.len()));
        self.tabs.insert(at, tab);
        self.active = Some(at);
        at
    }
}

#[derive(Clone, Debug, PartialEq)]
pub struct PaneLayout<T> {
    panes: Vec<Pane<T>>,
    flexes: Vec<f32>,
    active: usize,
    next_id: u64,
}

impl<T> Default for PaneLayout<T> {
    fn default() -> Self {
        Self {
            panes: vec![Pane {
                id: PaneId(0),
                tabs: Vec::new(),
                active: None,
            }],
            flexes: vec![1.0],
            active: 0,
            next_id: 1,
        }
    }
}

impl<T> PaneLayout<T> {
    #[must_use]
    pub fn panes(&self) -> &[Pane<T>] {
        &self.panes
    }

    #[must_use]
    pub fn flexes(&self) -> &[f32] {
        &self.flexes
    }

    #[must_use]
    pub const fn active_pane(&self) -> usize {
        self.active
    }

    #[must_use]
    pub fn active_tab(&self) -> Option<&Tab<T>> {
        self.panes.get(self.active)?.active_tab()
    }

    pub fn items(&self) -> impl Iterator<Item = &T> {
        self.panes
            .iter()
            .flat_map(|pane| pane.tabs.iter().map(|tab| &tab.item))
    }

    #[must_use]
    pub fn position(&self, wanted: impl Fn(&T) -> bool) -> Option<(usize, usize)> {
        self.panes.iter().enumerate().find_map(|(pane, entry)| {
            entry
                .tabs
                .iter()
                .position(|tab| wanted(&tab.item))
                .map(|tab| (pane, tab))
        })
    }

    fn new_pane(&mut self, tabs: Vec<Tab<T>>, active: Option<usize>) -> Pane<T> {
        let id = PaneId(self.next_id);
        self.next_id += 1;
        Pane { id, tabs, active }
    }

    pub fn restore(&mut self, panes: Vec<(Vec<Tab<T>>, usize)>, flexes: &[f32], active: usize) {
        let mut restored = Vec::new();
        for (tabs, active_tab) in panes {
            if let Some(last) = tabs.len().checked_sub(1) {
                let pane = self.new_pane(tabs, Some(active_tab.min(last)));
                restored.push(pane);
            }
        }
        if restored.is_empty() {
            return;
        }
        self.flexes = if flexes.len() == restored.len() && flexes.iter().all(|flex| *flex > 0.0) {
            flexes.to_vec()
        } else {
            vec![1.0; restored.len()]
        };
        self.active = active.min(restored.len() - 1);
        self.panes = restored;
    }

    pub fn focus_pane(&mut self, pane: usize) -> bool {
        let Some(target) = self.panes.get_mut(pane) else {
            return false;
        };
        if let Some(last) = target.tabs.len().checked_sub(1) {
            target.active = Some(target.active.unwrap_or(0).min(last));
        }
        let changed = self.active != pane;
        self.active = pane;
        changed
    }

    pub fn switch(&mut self, pane: usize, tab: usize) {
        if let Some(target) = self.panes.get_mut(pane)
            && tab < target.tabs.len()
        {
            target.active = Some(tab);
        }
    }

    pub fn open(&mut self, pane: usize, at: Option<usize>, item: T) -> usize {
        let pane = pane.min(self.panes.len() - 1);
        self.active = pane;
        self.panes[pane].insert(Tab::unpinned(item), at)
    }

    pub fn open_split(&mut self, reference: usize, side: Side, item: T) {
        let insert_at = self.split_flexes(reference, side);
        let pane = self.new_pane(vec![Tab::unpinned(item)], Some(0));
        self.panes.insert(insert_at, pane);
        self.active = insert_at;
    }

    fn split_flexes(&mut self, reference: usize, side: Side) -> usize {
        let reference = reference.min(self.panes.len() - 1);
        let insert_at = match side {
            Side::Left => reference,
            Side::Right => reference + 1,
        };
        let half = self.flexes[reference] / 2.0;
        self.flexes[reference] = half;
        self.flexes.insert(insert_at, half);
        insert_at
    }

    fn remove_pane(&mut self, index: usize, flex_sink: Option<usize>) -> Pane<T> {
        let removed = self.flexes.remove(index);
        let pane = self.panes.remove(index);
        if !self.flexes.is_empty() {
            let sink = flex_sink.unwrap_or_else(|| index.saturating_sub(1));
            let last = self.flexes.len() - 1;
            self.flexes[sink.min(last)] += removed;
        }
        pane
    }

    pub fn close_pane(&mut self, pane: usize) -> Vec<T> {
        if self.panes.len() <= 1 || pane >= self.panes.len() {
            return Vec::new();
        }
        let removed = self.remove_pane(pane, None);
        if pane < self.active {
            self.active -= 1;
        }
        self.active = self.active.min(self.panes.len() - 1);
        removed.tabs.into_iter().map(|tab| tab.item).collect()
    }

    pub fn close(&mut self, pane: usize, tab: usize) -> Vec<T> {
        let Some(entry) = self.panes.get_mut(pane) else {
            return Vec::new();
        };
        let Some(closed) = entry.detach(tab) else {
            return Vec::new();
        };
        if entry.tabs.is_empty() {
            self.close_pane(pane);
        }
        vec![closed.item]
    }

    pub fn close_others(&mut self, pane: usize, keep: usize) -> Vec<T> {
        let Some(entry) = self.panes.get_mut(pane) else {
            return Vec::new();
        };
        if keep >= entry.tabs.len() {
            return Vec::new();
        }
        let mut closed = Vec::new();
        let mut kept = Vec::new();
        for (index, tab) in entry.tabs.drain(..).enumerate() {
            if index == keep {
                entry.active = Some(kept.len());
                kept.push(tab);
            } else if tab.pinned {
                kept.push(tab);
            } else {
                closed.push(tab.item);
            }
        }
        entry.tabs = kept;
        closed
    }

    pub fn close_all(&mut self, pane: usize) -> Vec<T> {
        let Some(entry) = self.panes.get_mut(pane) else {
            return Vec::new();
        };
        let (pinned, closed): (Vec<Tab<T>>, Vec<Tab<T>>) =
            entry.tabs.drain(..).partition(|tab| tab.pinned);
        entry.active = (!pinned.is_empty()).then_some(0);
        entry.tabs = pinned;
        if entry.tabs.is_empty() {
            self.close_pane(pane);
        }
        closed.into_iter().map(|tab| tab.item).collect()
    }

    pub fn toggle_pin(&mut self, pane: usize, tab: usize) {
        let Some(entry) = self.panes.get_mut(pane) else {
            return;
        };
        let Some(target) = entry.tabs.get_mut(tab) else {
            return;
        };
        target.pinned = !target.pinned;
        let mut order: Vec<usize> = (0..entry.tabs.len()).collect();
        order.sort_by_key(|index| !entry.tabs[*index].pinned);
        let active = entry
            .active
            .and_then(|active| order.iter().position(|index| *index == active));
        let mut slots: Vec<Option<Tab<T>>> = entry.tabs.drain(..).map(Some).collect();
        entry.tabs = order
            .into_iter()
            .filter_map(|index| slots[index].take())
            .collect();
        entry.active = active;
    }

    pub fn move_within(&mut self, pane: usize, from: usize, gap: usize) {
        let Some(entry) = self.panes.get_mut(pane) else {
            return;
        };
        if from >= entry.tabs.len() {
            return;
        }
        let droppable = reorder_tab_gap(&entry.tabs, from, gap);
        if droppable == from || droppable == from + 1 {
            return;
        }
        let tab = entry.tabs.remove(from);
        let to = if droppable > from {
            droppable - 1
        } else {
            droppable
        };
        entry.tabs.insert(to, tab);
        entry.active = entry.active.map(|active| match active {
            _ if active == from => to,
            _ if from < active && active <= to => active - 1,
            _ if to <= active && active < from => active + 1,
            _ => active,
        });
    }

    pub fn move_to_pane(
        &mut self,
        source: usize,
        tab: usize,
        destination: usize,
        at: Option<usize>,
    ) {
        if source == destination || destination >= self.panes.len() {
            return;
        }
        let Some(detached) = self.panes.get_mut(source).and_then(|pane| pane.detach(tab)) else {
            return;
        };
        let mut destination = destination;
        if self.panes[source].tabs.is_empty() {
            if source < destination {
                destination -= 1;
            }
            self.remove_pane(source, None);
        }
        self.panes[destination].insert(detached, at);
        self.active = destination;
    }

    pub fn split_tab(&mut self, source: usize, tab: usize, reference: usize, side: Side) {
        if reference >= self.panes.len() {
            return;
        }
        let Some(detached) = self.panes.get_mut(source).and_then(|pane| pane.detach(tab)) else {
            return;
        };
        let mut reference = reference;
        if self.panes[source].tabs.is_empty() {
            if source < reference {
                reference -= 1;
            }
            self.remove_pane(source, Some(reference));
        }
        let insert_at = self.split_flexes(reference, side);
        let pane = self.new_pane(vec![detached], Some(0));
        self.panes.insert(insert_at, pane);
        self.active = insert_at;
    }

    pub fn remove_where(&mut self, doomed: impl Fn(&T) -> bool) -> Vec<T> {
        let mut removed = Vec::new();
        for pane in &mut self.panes {
            let active = pane.active;
            let active_survives = pane.active_tab().is_some_and(|tab| !doomed(&tab.item));
            let mut kept = Vec::new();
            let mut next_active = None;
            for (index, tab) in pane.tabs.drain(..).enumerate() {
                if doomed(&tab.item) {
                    removed.push(tab.item);
                } else {
                    if active_survives && active == Some(index) {
                        next_active = Some(kept.len());
                    }
                    kept.push(tab);
                }
            }
            pane.active = kept
                .len()
                .checked_sub(1)
                .map(|last| next_active.unwrap_or_else(|| active.unwrap_or(0).min(last)));
            pane.tabs = kept;
        }
        removed
    }

    pub fn resize(&mut self, divider: usize, left: f32, pair: f32, min: f32) {
        if divider + 1 >= self.flexes.len() || pair <= 2.0 * min {
            return;
        }
        let left = left.clamp(min, pair - min);
        let sum = self.flexes[divider] + self.flexes[divider + 1];
        self.flexes[divider] = sum * left / pair;
        self.flexes[divider + 1] = sum - self.flexes[divider];
    }
}
