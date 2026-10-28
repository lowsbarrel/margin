#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum EditOrigin {
    Typing,
    Deletion,
    Paste,
    Command,
}
