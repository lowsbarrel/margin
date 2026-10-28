use std::fmt;

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct SessionId(u32);

impl SessionId {
    #[must_use]
    pub const fn new(id: u32) -> Self {
        Self(id)
    }
}

impl fmt::Display for SessionId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.0)
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct TerminalSize {
    cols: u16,
    rows: u16,
}

impl TerminalSize {
    #[must_use]
    pub const fn new(cols: u16, rows: u16) -> Self {
        Self {
            cols: at_least_one(cols),
            rows: at_least_one(rows),
        }
    }

    #[must_use]
    pub const fn cols(self) -> u16 {
        self.cols
    }

    #[must_use]
    pub const fn rows(self) -> u16 {
        self.rows
    }
}

const fn at_least_one(value: u16) -> u16 {
    if value == 0 { 1 } else { value }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_size_never_collapses_to_zero() {
        assert_eq!(TerminalSize::new(0, 0), TerminalSize::new(1, 1));
        assert_eq!(TerminalSize::new(120, 40).cols(), 120);
        assert_eq!(TerminalSize::new(120, 40).rows(), 40);
    }

    #[test]
    fn a_session_id_prints_as_its_number() {
        assert_eq!(SessionId::new(7).to_string(), "7");
    }
}
