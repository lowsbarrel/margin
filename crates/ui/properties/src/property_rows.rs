use std::collections::HashMap;
use std::mem;
use std::slice::Iter;

use gpui_kit::{Context, Window};
use margin_markdown::{Frontmatter, PropertyKind};

use crate::effect::{CaretAt, FocusTarget};
use crate::properties_panel::PropertiesPanel;
use crate::property_edits::effective_kind;
use crate::property_row::Row;

#[derive(Default)]
pub(crate) struct Rows {
    rows: Vec<Row>,
    kinds: HashMap<String, PropertyKind>,
}

impl Rows {
    pub(crate) fn sync(
        &mut self,
        frontmatter: Option<&Frontmatter>,
        window: &mut Window,
        cx: &mut Context<'_, PropertiesPanel>,
    ) {
        let mut old = mem::take(&mut self.rows);
        let properties = frontmatter
            .filter(|frontmatter| !frontmatter.unsupported)
            .map(|frontmatter| frontmatter.properties.as_slice())
            .unwrap_or_default();
        for property in properties {
            let kind = effective_kind(property, self.kinds.get(&property.key).copied());
            let row = match old.iter().position(|row| row.is_same(property, kind)) {
                Some(index) => old.swap_remove(index),
                None => Row::new(property.clone(), kind, window, cx),
            };
            self.rows.push(row);
        }
    }

    pub(crate) fn iter(&self) -> Iter<'_, Row> {
        self.rows.iter()
    }

    pub(crate) fn len(&self) -> usize {
        self.rows.len()
    }

    pub(crate) fn get(&self, index: usize) -> Option<&Row> {
        self.rows.get(index)
    }

    pub(crate) fn last(&self) -> Option<&Row> {
        self.rows.last()
    }

    pub(crate) fn find(&self, key: &str) -> Option<&Row> {
        self.rows.iter().find(|row| row.key() == key)
    }

    pub(crate) fn find_mut(&mut self, key: &str) -> Option<&mut Row> {
        self.rows.iter_mut().find(|row| row.key() == key)
    }

    pub(crate) fn keys(&self) -> Vec<String> {
        self.rows.iter().map(|row| row.key().to_owned()).collect()
    }

    pub(crate) fn neighbour_after_removing(&self, keys: &[&str]) -> FocusTarget {
        let left: Vec<&Row> = self
            .rows
            .iter()
            .filter(|row| !keys.contains(&row.key()))
            .collect();
        self.rows
            .iter()
            .position(|row| keys.contains(&row.key()))
            .and_then(|index| left.get(index.min(left.len().saturating_sub(1))))
            .map_or(FocusTarget::Body, |row| {
                FocusTarget::Row(row.key().to_owned())
            })
    }

    pub(crate) fn choose_kind(&mut self, key: &str, kind: PropertyKind) {
        self.kinds.insert(key.to_owned(), kind);
    }

    pub(crate) fn carry_kind(&mut self, from: &str, to: &str) {
        if let Some(kind) = self.kinds.get(from).copied() {
            self.kinds.insert(to.to_owned(), kind);
        }
    }

    pub(crate) fn focus(
        &mut self,
        target: &FocusTarget,
        window: &mut Window,
        cx: &mut Context<'_, PropertiesPanel>,
    ) {
        match target {
            FocusTarget::Row(key) => {
                if let Some(row) = self.find(key) {
                    row.focus_handle().focus(window, cx);
                }
            }
            FocusTarget::Value(key) => {
                if let Some(row) = self.find_mut(key) {
                    row.focus_value(CaretAt::End, window, cx);
                }
            }
            FocusTarget::Pill(key, index) => {
                if let Some(pills) = self.find_mut(key).and_then(Row::pills_mut) {
                    pills.focus_pill(*index, window, cx);
                }
            }
            FocusTarget::Add | FocusTarget::New | FocusTarget::Body => {}
        }
    }
}
