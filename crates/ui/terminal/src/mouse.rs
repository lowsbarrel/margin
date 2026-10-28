use alacritty_terminal::index::Point;
use alacritty_terminal::term::TermMode;
use gpui_kit::{Modifiers, MouseButton};

const MOTION: u8 = 32;
const RELEASE: u8 = 3;
const WHEEL_UP: u8 = 64;
const WHEEL_DOWN: u8 = 65;
const NORMAL_OFFSET: u32 = 32;
const NORMAL_LIMIT: u32 = 223;
const UTF8_LIMIT: u32 = 2015;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum MouseAction {
    Press(MouseButton),
    Release(MouseButton),
    Drag(MouseButton),
    Move,
    WheelUp,
    WheelDown,
}

pub(crate) fn reports_mouse(mode: TermMode, modifiers: Modifiers) -> bool {
    mode.intersects(TermMode::MOUSE_MODE) && !modifiers.shift
}

pub(crate) fn mouse_report(
    action: MouseAction,
    position: Point<usize>,
    modifiers: Modifiers,
    mode: TermMode,
) -> Option<Vec<u8>> {
    let wanted = match action {
        MouseAction::Drag(_) => mode.intersects(TermMode::MOUSE_DRAG | TermMode::MOUSE_MOTION),
        MouseAction::Move => mode.contains(TermMode::MOUSE_MOTION),
        _ => mode.intersects(TermMode::MOUSE_MODE),
    };
    if !wanted {
        return None;
    }
    let code = match action {
        MouseAction::Press(button) | MouseAction::Release(button) => button_code(button)?,
        MouseAction::Drag(button) => button_code(button)? + MOTION,
        MouseAction::Move => RELEASE + MOTION,
        MouseAction::WheelUp => WHEEL_UP,
        MouseAction::WheelDown => WHEEL_DOWN,
    } + modifier_bits(modifiers);
    let released = matches!(action, MouseAction::Release(_));
    if mode.contains(TermMode::SGR_MOUSE) {
        let suffix = if released { 'm' } else { 'M' };
        let column = position.column.0 + 1;
        let row = position.line + 1;
        return Some(format!("\x1b[<{code};{column};{row}{suffix}").into_bytes());
    }
    let code = if released {
        RELEASE + modifier_bits(modifiers)
    } else {
        code
    };
    let utf8 = mode.contains(TermMode::UTF8_MOUSE);
    let mut bytes = b"\x1b[M".to_vec();
    push_coordinate(&mut bytes, u32::from(code), utf8)?;
    push_coordinate(&mut bytes, coordinate(position.column.0)?, utf8)?;
    push_coordinate(&mut bytes, coordinate(position.line)?, utf8)?;
    Some(bytes)
}

fn coordinate(value: usize) -> Option<u32> {
    u32::try_from(value).ok()?.checked_add(1)
}

fn push_coordinate(bytes: &mut Vec<u8>, value: u32, utf8: bool) -> Option<()> {
    let limit = if utf8 { UTF8_LIMIT } else { NORMAL_LIMIT };
    if value > limit {
        return None;
    }
    let encoded = value + NORMAL_OFFSET;
    if utf8 {
        let character = char::from_u32(encoded)?;
        let mut buffer = [0; 4];
        bytes.extend_from_slice(character.encode_utf8(&mut buffer).as_bytes());
    } else {
        bytes.push(u8::try_from(encoded).ok()?);
    }
    Some(())
}

const fn button_code(button: MouseButton) -> Option<u8> {
    match button {
        MouseButton::Left => Some(0),
        MouseButton::Middle => Some(1),
        MouseButton::Right => Some(2),
        MouseButton::Navigate(_) => None,
    }
}

fn modifier_bits(modifiers: Modifiers) -> u8 {
    4 * u8::from(modifiers.shift) + 8 * u8::from(modifiers.alt) + 16 * u8::from(modifiers.control)
}

#[cfg(test)]
mod tests {
    use alacritty_terminal::index::Column;

    use super::*;

    const AT: Point<usize> = Point {
        line: 1,
        column: Column(4),
    };

    #[test]
    fn sgr_reports_press_and_release_with_one_based_cells() {
        let mode = TermMode::MOUSE_REPORT_CLICK | TermMode::SGR_MOUSE;
        let none = Modifiers::none();
        let press = mouse_report(MouseAction::Press(MouseButton::Left), AT, none, mode);
        assert_eq!(press, Some(b"\x1b[<0;5;2M".to_vec()));
        let release = mouse_report(MouseAction::Release(MouseButton::Left), AT, none, mode);
        assert_eq!(release, Some(b"\x1b[<0;5;2m".to_vec()));
        let wheel = mouse_report(MouseAction::WheelDown, AT, none, mode);
        assert_eq!(wheel, Some(b"\x1b[<65;5;2M".to_vec()));
    }

    #[test]
    fn normal_reports_offset_by_32_and_skip_unrequested_motion() {
        let mode = TermMode::MOUSE_REPORT_CLICK;
        let ctrl = Modifiers::control();
        let press = mouse_report(MouseAction::Press(MouseButton::Right), AT, ctrl, mode);
        assert_eq!(
            press,
            Some(vec![0x1b, b'[', b'M', 32 + 2 + 16, 32 + 5, 32 + 2])
        );
        assert_eq!(
            mouse_report(MouseAction::Drag(MouseButton::Left), AT, ctrl, mode),
            None
        );
    }
}
