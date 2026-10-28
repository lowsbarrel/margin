use std::collections::HashSet;

const INVALID_CHARACTERS: [char; 9] = ['/', '\\', ':', '*', '?', '"', '<', '>', '|'];
const RESERVED_NAMES: [&str; 22] = [
    "CON", "PRN", "AUX", "NUL", "COM1", "COM2", "COM3", "COM4", "COM5", "COM6", "COM7", "COM8",
    "COM9", "LPT1", "LPT2", "LPT3", "LPT4", "LPT5", "LPT6", "LPT7", "LPT8", "LPT9",
];

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum NameError {
    Empty,
    NotAscii,
    InvalidCharacters,
    DotStart,
    Trailing,
    Reserved,
}

impl NameError {
    pub(crate) const fn message_key(self) -> &'static str {
        match self {
            Self::Empty => "validate_name_empty",
            Self::NotAscii => "validate_name_ascii",
            Self::InvalidCharacters => "validate_name_invalid_chars",
            Self::DotStart => "validate_name_dot_start",
            Self::Trailing => "validate_name_trailing",
            Self::Reserved => "validate_name_reserved",
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum EntryType {
    File,
    Folder,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum CollisionSuffix {
    Numeric,
    Copy,
}

pub(crate) fn validate_name(name: &str) -> Result<&str, NameError> {
    let trimmed = name.trim();
    if trimmed.is_empty() {
        return Err(NameError::Empty);
    }
    if !trimmed
        .chars()
        .all(|character| matches!(character, ' '..='~'))
    {
        return Err(NameError::NotAscii);
    }
    if trimmed.contains(INVALID_CHARACTERS) {
        return Err(NameError::InvalidCharacters);
    }
    if trimmed.starts_with('.') {
        return Err(NameError::DotStart);
    }
    if trimmed.ends_with([' ', '.']) {
        return Err(NameError::Trailing);
    }
    let stem = trimmed.rsplit_once('.').map_or(trimmed, |(stem, _)| stem);
    if RESERVED_NAMES.contains(&stem.to_ascii_uppercase().as_str()) {
        return Err(NameError::Reserved);
    }
    Ok(trimmed)
}

pub(crate) fn normalize_name(input: &str, entry: EntryType) -> Result<String, NameError> {
    let name = validate_name(input)?;
    Ok(match entry {
        EntryType::File if !name.contains('.') => format!("{name}.md"),
        EntryType::File | EntryType::Folder => name.to_owned(),
    })
}

pub(crate) fn unique_name<'name>(
    name: &str,
    existing: impl IntoIterator<Item = &'name str>,
    suffix: CollisionSuffix,
) -> String {
    let (stem, extension) = match name.rfind('.') {
        Some(index) if index > 0 => name.split_at(index),
        _ => (name, ""),
    };
    let taken: HashSet<String> = existing.into_iter().map(str::to_lowercase).collect();
    let is_free = |candidate: &String| !taken.contains(&candidate.to_lowercase());
    let numbered = |index: usize| match (suffix, index) {
        (CollisionSuffix::Copy, 1) => format!("{stem} copy{extension}"),
        (CollisionSuffix::Copy, _) => format!("{stem} copy {index}{extension}"),
        (CollisionSuffix::Numeric, _) => format!("{stem} {index}{extension}"),
    };
    if suffix == CollisionSuffix::Numeric && is_free(&name.to_owned()) {
        return name.to_owned();
    }
    (1..=taken.len() + 1)
        .map(numbered)
        .find(is_free)
        .expect("one more candidate than taken names always finds a free one")
}

pub(crate) fn stem_length(name: &str) -> usize {
    match name.rfind('.') {
        Some(index) if index > 0 => index,
        _ => name.len(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn rejects_names_the_original_rejected() {
        assert_eq!(validate_name("  "), Err(NameError::Empty));
        assert_eq!(validate_name("café"), Err(NameError::NotAscii));
        assert_eq!(validate_name("a/b"), Err(NameError::InvalidCharacters));
        assert_eq!(validate_name("a:b"), Err(NameError::InvalidCharacters));
        assert_eq!(validate_name(".hidden"), Err(NameError::DotStart));
        assert_eq!(validate_name("name."), Err(NameError::Trailing));
        assert_eq!(validate_name("con.md"), Err(NameError::Reserved));
        assert_eq!(validate_name("Lpt9"), Err(NameError::Reserved));
        assert_eq!(validate_name("  Plan.md "), Ok("Plan.md"));
        assert_eq!(validate_name("console.md"), Ok("console.md"));
    }

    #[test]
    fn files_without_an_extension_become_notes() {
        assert_eq!(
            normalize_name("Plan", EntryType::File),
            Ok("Plan.md".to_owned())
        );
        assert_eq!(
            normalize_name("a.canvas", EntryType::File),
            Ok("a.canvas".to_owned())
        );
        assert_eq!(
            normalize_name("Plan", EntryType::Folder),
            Ok("Plan".to_owned())
        );
    }

    #[test]
    fn numbers_untitled_notes_case_insensitively() {
        assert_eq!(
            unique_name("Untitled.md", [], CollisionSuffix::Numeric),
            "Untitled.md"
        );
        assert_eq!(
            unique_name(
                "Untitled.md",
                ["untitled.MD", "Untitled 1.md"],
                CollisionSuffix::Numeric
            ),
            "Untitled 2.md"
        );
        assert_eq!(
            unique_name("Folder", ["Folder"], CollisionSuffix::Numeric),
            "Folder 1"
        );
    }

    #[test]
    fn duplicates_always_get_a_copy_suffix() {
        assert_eq!(
            unique_name("a.md", ["a.md"], CollisionSuffix::Copy),
            "a copy.md"
        );
        assert_eq!(
            unique_name("a.md", ["a.md", "a copy.md"], CollisionSuffix::Copy),
            "a copy 2.md"
        );
        assert_eq!(unique_name(".env", [], CollisionSuffix::Copy), ".env copy");
    }

    #[test]
    fn stem_length_stops_before_the_extension() {
        assert_eq!(stem_length("Plan.md"), 4);
        assert_eq!(stem_length("Folder"), 6);
    }
}
