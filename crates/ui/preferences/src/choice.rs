use gpui_kit::SharedString;
use gpui_kit::component::searchable_list::SearchableListItem;

#[derive(Clone, Debug)]
pub(crate) struct Choice<V> {
    label: SharedString,
    value: V,
}

impl<V> Choice<V> {
    pub(crate) const fn new(label: SharedString, value: V) -> Self {
        Self { label, value }
    }
}

impl<V: Clone + PartialEq> SearchableListItem for Choice<V> {
    type Value = V;

    fn title(&self) -> SharedString {
        self.label.clone()
    }

    fn value(&self) -> &Self::Value {
        &self.value
    }
}
