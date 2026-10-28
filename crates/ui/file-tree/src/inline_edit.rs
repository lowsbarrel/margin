use gpui_kit::component::input::{InputEvent, InputState};
use gpui_kit::{App, AppContext, Context, Entity, SharedString, Subscription, Window};
use margin_files::TreeRow;
use margin_i18n::t;
use margin_kernel::RelativePath;

use crate::file_tree::FileTree;
use crate::names::{self, EntryType, NameError};

#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) enum EditTarget {
    Rename {
        path: RelativePath,
        entry: EntryType,
    },
    NewFolder {
        parent: RelativePath,
    },
}

#[derive(Debug, PartialEq, Eq)]
pub(crate) enum Decision {
    Close,
    Invalid(NameError),
    CreateFolder { parent: RelativePath, name: String },
    Rename { path: RelativePath, name: String },
}

impl EditTarget {
    pub(crate) fn decide(&self, value: &str) -> Decision {
        let value = value.trim();
        match self {
            Self::NewFolder { .. } | Self::Rename { .. } if value.is_empty() => Decision::Close,
            Self::NewFolder { parent } => match names::validate_name(value) {
                Ok(name) => Decision::CreateFolder {
                    parent: parent.clone(),
                    name: name.to_owned(),
                },
                Err(error) => Decision::Invalid(error),
            },
            Self::Rename { path, .. } if value == path.file_name() => Decision::Close,
            Self::Rename { path, entry } => match names::normalize_name(value, *entry) {
                Ok(name) if name == path.file_name() => Decision::Close,
                Ok(name) => Decision::Rename {
                    path: path.clone(),
                    name,
                },
                Err(error) => Decision::Invalid(error),
            },
        }
    }
}

#[derive(Debug)]
pub(crate) struct InlineEdit {
    target: EditTarget,
    input: Entity<InputState>,
    submitting: bool,
    _events: Subscription,
}

impl InlineEdit {
    pub(crate) fn rename(
        row: &TreeRow,
        window: &mut Window,
        cx: &mut Context<'_, FileTree>,
    ) -> Self {
        let entry = if row.is_dir {
            EntryType::Folder
        } else {
            EntryType::File
        };
        let path = row.path.clone();
        Self::new(EditTarget::Rename { path, entry }, window, cx)
    }

    pub(crate) fn new(
        target: EditTarget,
        window: &mut Window,
        cx: &mut Context<'_, FileTree>,
    ) -> Self {
        let (value, placeholder) = match &target {
            EditTarget::Rename { path, .. } => (path.file_name().to_owned(), None),
            EditTarget::NewFolder { .. } => (String::new(), Some(t(cx, "folder_name_placeholder"))),
        };
        let selected = names::stem_length(&value);
        let input = cx.new(|cx| {
            let state = InputState::new(window, cx).default_value(SharedString::from(value));
            match placeholder {
                Some(placeholder) => state.placeholder(placeholder),
                None => state,
            }
        });
        input.update(cx, |state, cx| {
            state.focus(window, cx);
            state.set_selected_range(0..selected, cx);
        });
        let events = cx.subscribe_in(&input, window, |tree, _, event, window, cx| {
            if matches!(event, InputEvent::PressEnter { .. } | InputEvent::Blur) {
                tree.commit_edit(window, cx);
            }
        });
        Self {
            target,
            input,
            submitting: false,
            _events: events,
        }
    }

    pub(crate) const fn target(&self) -> &EditTarget {
        &self.target
    }

    pub(crate) const fn input(&self) -> &Entity<InputState> {
        &self.input
    }

    pub(crate) fn renames(&self, candidate: &RelativePath) -> bool {
        matches!(&self.target, EditTarget::Rename { path, .. } if path == candidate)
    }

    pub(crate) fn decide(&self, cx: &App) -> Decision {
        self.target.decide(&self.input.read(cx).value())
    }

    pub(crate) const fn is_submitting(&self) -> bool {
        self.submitting
    }

    pub(crate) const fn set_submitting(&mut self, submitting: bool) {
        self.submitting = submitting;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn path(raw: &str) -> RelativePath {
        RelativePath::new(raw).expect("valid path")
    }

    fn rename(raw: &str, entry: EntryType) -> EditTarget {
        EditTarget::Rename {
            path: path(raw),
            entry,
        }
    }

    #[test]
    fn unchanged_or_empty_names_close_the_editor() {
        let target = rename("notes/Plan.md", EntryType::File);
        assert_eq!(target.decide("  "), Decision::Close);
        assert_eq!(target.decide("Plan.md"), Decision::Close);
        assert_eq!(target.decide("Plan"), Decision::Close);
    }

    #[test]
    fn renames_with_a_note_extension_by_default() {
        let target = rename("notes/Plan.md", EntryType::File);
        assert_eq!(
            target.decide(" Roadmap "),
            Decision::Rename {
                path: path("notes/Plan.md"),
                name: "Roadmap.md".to_owned()
            }
        );
        let folder = rename("notes", EntryType::Folder);
        assert_eq!(
            folder.decide("archive"),
            Decision::Rename {
                path: path("notes"),
                name: "archive".to_owned()
            }
        );
    }

    #[test]
    fn invalid_names_report_the_rule_they_break() {
        let target = rename("Plan.md", EntryType::File);
        assert_eq!(
            target.decide("a?b"),
            Decision::Invalid(NameError::InvalidCharacters)
        );
        let folder = EditTarget::NewFolder {
            parent: RelativePath::root(),
        };
        assert_eq!(
            folder.decide(".git"),
            Decision::Invalid(NameError::DotStart)
        );
        assert_eq!(
            folder.decide("Projects"),
            Decision::CreateFolder {
                parent: RelativePath::root(),
                name: "Projects".to_owned()
            }
        );
    }
}
