#![expect(
    clippy::derive_partial_eq_without_eq,
    reason = "gpui_kit::actions! derives PartialEq without Eq"
)]

use std::path::PathBuf;

use gpui_kit::{App, Global, actions};
use margin_files::SortBy;
use margin_kernel::RelativePath;
use margin_workbench::binding;

use crate::model::{Gesture, Key};
use crate::operations::ClipboardMode;
use crate::selection::SelectedEntry;

pub(crate) const TREE_CONTEXT: &str = "FileTree";

actions!(
    file_tree,
    [
        SelectNext,
        SelectPrevious,
        ExpandFolder,
        CollapseFolder,
        OpenEntry,
        RenameEntry,
        DeleteEntry,
        CopyEntries,
        CutEntries,
        PasteEntries,
        SelectAllEntries,
    ]
);

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum NewEntry {
    Note,
    Canvas,
    Folder,
}

impl NewEntry {
    pub(crate) const fn file_name(self) -> &'static str {
        match self {
            Self::Note => "Untitled.md",
            Self::Canvas => "Untitled.canvas",
            Self::Folder => "",
        }
    }
}

#[derive(Clone, Debug)]
pub(crate) enum Command {
    Click(RelativePath, Gesture),
    ClickBackground,
    Key(Key),
    ContextMenu(RelativePath),
    Create(NewEntry, RelativePath),
    Rename(RelativePath),
    Duplicate(SelectedEntry),
    Reveal(RelativePath),
    Clipboard(ClipboardMode, Option<SelectedEntry>),
    Paste(RelativePath),
    Delete(SelectedEntry),
    Move(Vec<RelativePath>, RelativePath),
    Import(Vec<PathBuf>, RelativePath),
    CollapseAll,
    Sort(SortBy),
    SelectAll,
}

struct KeysBound;

impl Global for KeysBound {}

pub(crate) fn bind_keys_once(cx: &mut App) {
    if cx.has_global::<KeysBound>() {
        return;
    }
    cx.set_global(KeysBound);
    let context = Some(TREE_CONTEXT);
    let bindings = [
        binding("down", SelectNext, context, cx),
        binding("up", SelectPrevious, context, cx),
        binding("right", ExpandFolder, context, cx),
        binding("left", CollapseFolder, context, cx),
        binding("enter", OpenEntry, context, cx),
        binding("f2", RenameEntry, context, cx),
        binding("delete", DeleteEntry, context, cx),
        binding("backspace", DeleteEntry, context, cx),
        binding("secondary-c", CopyEntries, context, cx),
        binding("secondary-x", CutEntries, context, cx),
        binding("secondary-v", PasteEntries, context, cx),
        binding("secondary-a", SelectAllEntries, context, cx),
    ];
    cx.bind_keys(bindings);
}
