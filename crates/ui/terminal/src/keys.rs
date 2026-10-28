use alacritty_terminal::grid::Scroll;
use alacritty_terminal::term::TermMode;
use gpui_kit::{Keystroke, Modifiers};

const ESC: u8 = 0x1b;
const DELETE: u8 = 0x7f;
const BACKSPACE: u8 = 0x08;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum AltKey {
    Meta,
    ComposesCharacters,
}

pub(crate) fn scrollback_key(keystroke: &Keystroke, mode: TermMode) -> Option<Scroll> {
    let modifiers = keystroke.modifiers;
    if !modifiers.shift || modifiers.control || mode.contains(TermMode::ALT_SCREEN) {
        return None;
    }
    match keystroke.key.as_str() {
        "pageup" => Some(Scroll::PageUp),
        "pagedown" => Some(Scroll::PageDown),
        "home" => Some(Scroll::Top),
        "end" => Some(Scroll::Bottom),
        _ => None,
    }
}

pub(crate) fn key_bytes(keystroke: &Keystroke, mode: TermMode, alt: AltKey) -> Option<Vec<u8>> {
    let modifiers = keystroke.modifiers;
    if modifiers.platform {
        return None;
    }
    let code = modifier_code(modifiers);
    let meta = modifiers.alt && alt == AltKey::Meta;
    let bytes = match keystroke.key.as_str() {
        "up" => cursor_key(b'A', code, mode),
        "down" => cursor_key(b'B', code, mode),
        "right" if code == 3 => word_motion(b'f', b'C', alt),
        "left" if code == 3 => word_motion(b'b', b'D', alt),
        "right" => cursor_key(b'C', code, mode),
        "left" => cursor_key(b'D', code, mode),
        "home" => cursor_key(b'H', code, mode),
        "end" => cursor_key(b'F', code, mode),
        "insert" => tilde_key(2, code),
        "delete" => tilde_key(3, code),
        "pageup" => tilde_key(5, code),
        "pagedown" => tilde_key(6, code),
        "f1" => function_key(b'P', code),
        "f2" => function_key(b'Q', code),
        "f3" => function_key(b'R', code),
        "f4" => function_key(b'S', code),
        "f5" => tilde_key(15, code),
        "f6" => tilde_key(17, code),
        "f7" => tilde_key(18, code),
        "f8" => tilde_key(19, code),
        "f9" => tilde_key(20, code),
        "f10" => tilde_key(21, code),
        "f11" => tilde_key(23, code),
        "f12" => tilde_key(24, code),
        "enter" => with_meta(meta, b"\r"),
        "tab" if modifiers.shift => b"\x1b[Z".to_vec(),
        "tab" => with_meta(meta, b"\t"),
        "escape" => with_meta(meta, &[ESC]),
        "backspace" if modifiers.control => with_meta(meta, &[BACKSPACE]),
        "backspace" => with_meta(meta, &[DELETE]),
        "space" if modifiers.control => with_meta(meta, &[0]),
        "space" if meta => with_meta(meta, b" "),
        key => return character_key(keystroke, key, meta),
    };
    Some(bytes)
}

fn character_key(keystroke: &Keystroke, key: &str, meta: bool) -> Option<Vec<u8>> {
    let modifiers = keystroke.modifiers;
    let mut chars = key.chars();
    let character = chars.next().filter(|_| chars.next().is_none())?;
    if modifiers.control {
        return control_byte(character).map(|byte| with_meta(meta, &[byte]));
    }
    if !meta {
        return None;
    }
    let typed = keystroke.key_char.clone().unwrap_or_else(|| {
        if modifiers.shift {
            character.to_uppercase().collect()
        } else {
            character.to_string()
        }
    });
    Some(with_meta(true, typed.as_bytes()))
}

fn control_byte(character: char) -> Option<u8> {
    let byte = match character.to_ascii_lowercase() {
        letter @ 'a'..='z' => u8::try_from(letter).ok()? - b'a' + 1,
        '@' | '2' | ' ' => 0,
        '[' | '3' => ESC,
        '\\' | '4' => 0x1c,
        ']' | '5' => 0x1d,
        '^' | '6' => 0x1e,
        '_' | '-' | '/' | '7' => 0x1f,
        '?' | '8' => DELETE,
        _ => return None,
    };
    Some(byte)
}

fn modifier_code(modifiers: Modifiers) -> u8 {
    1 + u8::from(modifiers.shift) + 2 * u8::from(modifiers.alt) + 4 * u8::from(modifiers.control)
}

fn cursor_key(letter: u8, code: u8, mode: TermMode) -> Vec<u8> {
    if code > 1 {
        return format!("\x1b[1;{code}{}", char::from(letter)).into_bytes();
    }
    let introducer = if mode.contains(TermMode::APP_CURSOR) {
        b'O'
    } else {
        b'['
    };
    vec![ESC, introducer, letter]
}

fn word_motion(mac: u8, letter: u8, alt: AltKey) -> Vec<u8> {
    match alt {
        AltKey::ComposesCharacters => vec![ESC, mac],
        AltKey::Meta => vec![ESC, b'[', b'1', b';', b'5', letter],
    }
}

fn tilde_key(number: u8, code: u8) -> Vec<u8> {
    if code > 1 {
        format!("\x1b[{number};{code}~").into_bytes()
    } else {
        format!("\x1b[{number}~").into_bytes()
    }
}

fn function_key(letter: u8, code: u8) -> Vec<u8> {
    if code > 1 {
        format!("\x1b[1;{code}{}", char::from(letter)).into_bytes()
    } else {
        vec![ESC, b'O', letter]
    }
}

fn with_meta(meta: bool, bytes: &[u8]) -> Vec<u8> {
    let mut output = Vec::with_capacity(bytes.len() + 1);
    if meta {
        output.push(ESC);
    }
    output.extend_from_slice(bytes);
    output
}

pub(crate) fn paste_bytes(text: &str, mode: TermMode) -> Vec<u8> {
    if mode.contains(TermMode::BRACKETED_PASTE) {
        let inner: String = text
            .chars()
            .filter(|c| !matches!(c, '\x1b' | '\x03'))
            .collect();
        return format!("\x1b[200~{inner}\x1b[201~").into_bytes();
    }
    text.replace("\r\n", "\r").replace('\n', "\r").into_bytes()
}

pub(crate) fn alternate_scroll(lines: i32, mode: TermMode) -> Vec<u8> {
    let letter = if lines > 0 { b'A' } else { b'B' };
    let arrow = cursor_key(letter, 1, mode);
    arrow.repeat(usize::try_from(lines.unsigned_abs()).unwrap_or(usize::MAX))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn bytes(source: &'static str, mode: TermMode, alt: AltKey) -> Option<Vec<u8>> {
        let keystroke = Keystroke::parse(source).expect("valid keystroke");
        key_bytes(&keystroke, mode, alt)
    }

    fn normal(source: &'static str) -> Option<Vec<u8>> {
        bytes(source, TermMode::default(), AltKey::Meta)
    }

    #[test]
    fn arrows_follow_the_cursor_key_mode() {
        assert_eq!(normal("up"), Some(b"\x1b[A".to_vec()));
        assert_eq!(normal("left"), Some(b"\x1b[D".to_vec()));
        let app = TermMode::default() | TermMode::APP_CURSOR;
        assert_eq!(bytes("up", app, AltKey::Meta), Some(b"\x1bOA".to_vec()));
        assert_eq!(bytes("home", app, AltKey::Meta), Some(b"\x1bOH".to_vec()));
        assert_eq!(normal("shift-up"), Some(b"\x1b[1;2A".to_vec()));
        assert_eq!(normal("ctrl-right"), Some(b"\x1b[1;5C".to_vec()));
    }

    #[test]
    fn alt_arrows_move_by_word() {
        assert_eq!(normal("alt-left"), Some(b"\x1b[1;5D".to_vec()));
        assert_eq!(
            bytes("alt-left", TermMode::default(), AltKey::ComposesCharacters),
            Some(b"\x1bb".to_vec())
        );
    }

    #[test]
    fn function_and_editing_keys_use_xterm_sequences() {
        assert_eq!(normal("f1"), Some(b"\x1bOP".to_vec()));
        assert_eq!(normal("shift-f1"), Some(b"\x1b[1;2P".to_vec()));
        assert_eq!(normal("f5"), Some(b"\x1b[15~".to_vec()));
        assert_eq!(normal("f12"), Some(b"\x1b[24~".to_vec()));
        assert_eq!(normal("ctrl-delete"), Some(b"\x1b[3;5~".to_vec()));
        assert_eq!(normal("pageup"), Some(b"\x1b[5~".to_vec()));
        assert_eq!(normal("shift-tab"), Some(b"\x1b[Z".to_vec()));
        assert_eq!(normal("backspace"), Some(vec![DELETE]));
        assert_eq!(normal("ctrl-backspace"), Some(vec![BACKSPACE]));
        assert_eq!(normal("enter"), Some(b"\r".to_vec()));
        assert_eq!(normal("escape"), Some(vec![ESC]));
    }

    #[test]
    fn control_chords_become_c0_bytes() {
        assert_eq!(normal("ctrl-c"), Some(vec![3]));
        assert_eq!(normal("ctrl-a"), Some(vec![1]));
        assert_eq!(normal("ctrl-shift-z"), Some(vec![26]));
        assert_eq!(normal("ctrl-["), Some(vec![ESC]));
        assert_eq!(normal("ctrl-space"), Some(vec![0]));
        assert_eq!(normal("ctrl-alt-x"), Some(vec![ESC, 24]));
    }

    #[test]
    fn alt_is_meta_unless_it_composes_characters() {
        assert_eq!(normal("alt-b"), Some(b"\x1bb".to_vec()));
        assert_eq!(normal("alt-shift-b"), Some(b"\x1bB".to_vec()));
        assert_eq!(
            bytes("alt-b", TermMode::default(), AltKey::ComposesCharacters),
            None
        );
    }

    #[test]
    fn plain_and_command_keys_are_left_to_text_input_and_bindings() {
        assert_eq!(normal("a"), None);
        assert_eq!(normal("shift-a"), None);
        assert_eq!(normal("cmd-c"), None);
    }

    #[test]
    fn pastes_normalise_newlines_and_honour_bracketed_mode() {
        assert_eq!(
            paste_bytes("a\r\nb\nc", TermMode::default()),
            b"a\rb\rc".to_vec()
        );
        let bracketed = TermMode::default() | TermMode::BRACKETED_PASTE;
        assert_eq!(
            paste_bytes("x\x1b[201~y", bracketed),
            b"\x1b[200~x[201~y\x1b[201~".to_vec()
        );
    }

    #[test]
    fn wheel_in_alternate_screen_sends_arrows() {
        assert_eq!(
            alternate_scroll(2, TermMode::default()),
            b"\x1b[A\x1b[A".to_vec()
        );
        let app = TermMode::default() | TermMode::APP_CURSOR;
        assert_eq!(alternate_scroll(-1, app), b"\x1bOB".to_vec());
    }
}
