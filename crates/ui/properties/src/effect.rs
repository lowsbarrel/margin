use margin_markdown::{PropertyKind, PropertyValue};

use crate::property_link::LinkTarget;

#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) enum FocusTarget {
    Add,
    New,
    Body,
    Row(String),
    Value(String),
    Pill(String, usize),
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum CaretAt {
    Start,
    End,
    All,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum FocusAfter {
    Nothing,
    Row,
    Value,
}

#[derive(Clone, Debug, PartialEq)]
pub(crate) enum Effect {
    Nothing,
    Commit(PropertyValue, Option<FocusTarget>),
    Rename(String, Option<FocusTarget>),
    ChangeKind(PropertyKind),
    Remove,
    Focus(FocusTarget),
    OpenLink(LinkTarget),
    OpenTag(String),
    Invalid(&'static str),
}
