use crate::operations::ClipboardMode;
use crate::selection::SelectedEntry;

#[derive(Debug, Default)]
pub(crate) struct Clipboard {
    content: Option<(ClipboardMode, Vec<SelectedEntry>)>,
}

impl Clipboard {
    pub(crate) const fn has_items(&self) -> bool {
        self.content.is_some()
    }

    pub(crate) fn store(
        &mut self,
        mode: ClipboardMode,
        entries: Vec<SelectedEntry>,
    ) -> Option<&'static str> {
        if entries.is_empty() {
            return None;
        }
        self.content = Some((mode, entries));
        Some(match mode {
            ClipboardMode::Copy => "toast_copied",
            ClipboardMode::Cut => "toast_cut_clipboard",
        })
    }

    pub(crate) fn take(&mut self) -> Option<(ClipboardMode, Vec<SelectedEntry>)> {
        match &self.content {
            Some((ClipboardMode::Cut, _)) => self.content.take(),
            other => other.clone(),
        }
    }
}
