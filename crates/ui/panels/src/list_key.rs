use gpui_kit::{App, KeyDownEvent};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum ListKey {
    Previous,
    Next,
    Activate,
    Commit,
    Delete,
    Close,
}

impl ListKey {
    pub(crate) fn of(event: &KeyDownEvent, cx: &App) -> Option<Self> {
        let keystroke = &event.keystroke;
        let secondary = keystroke.modifiers.secondary(cx);
        match keystroke.key.as_str() {
            "up" => Some(Self::Previous),
            "down" => Some(Self::Next),
            "enter" if secondary => Some(Self::Commit),
            "enter" => Some(Self::Activate),
            "escape" => Some(Self::Close),
            "delete" => Some(Self::Delete),
            "backspace" if secondary => Some(Self::Delete),
            _ => None,
        }
    }
}

pub(crate) fn step(selected: Option<usize>, len: usize, key: ListKey) -> Option<usize> {
    let last = len.checked_sub(1)?;
    Some(match (selected, key) {
        (None, ListKey::Previous) => last,
        (None, _) => 0,
        (Some(index), ListKey::Previous) => index.saturating_sub(1),
        (Some(index), ListKey::Next) => (index + 1).min(last),
        (Some(index), _) => index.min(last),
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn stepping_clamps_to_the_list_and_starts_from_the_matching_end() {
        assert_eq!(step(None, 0, ListKey::Next), None);
        assert_eq!(step(None, 3, ListKey::Next), Some(0));
        assert_eq!(step(None, 3, ListKey::Previous), Some(2));
        assert_eq!(step(Some(2), 3, ListKey::Next), Some(2));
        assert_eq!(step(Some(0), 3, ListKey::Previous), Some(0));
        assert_eq!(step(Some(5), 3, ListKey::Activate), Some(2));
    }
}
