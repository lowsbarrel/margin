use std::fmt;
use std::str::FromStr;

use serde::{Deserialize, Serialize};
use thiserror::Error;

#[derive(Clone, Debug, Error, PartialEq, Eq)]
#[non_exhaustive]
pub enum PathError {
    #[error("`{0}` has an empty, `.` or `..` component")]
    InvalidComponent(String),
    #[error("`{0}` contains a backslash or NUL character")]
    InvalidCharacter(String),
    #[error("the root has no file name")]
    Root,
}

#[derive(Clone, Debug, Default, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(try_from = "String", into = "String")]
pub struct RelativePath(String);

impl RelativePath {
    pub fn new(raw: &str) -> Result<Self, PathError> {
        let trimmed = raw.trim_matches('/');
        if trimmed.contains(['\\', '\0']) {
            return Err(PathError::InvalidCharacter(raw.to_owned()));
        }
        if trimmed.is_empty() {
            return Ok(Self::root());
        }
        let invalid = trimmed
            .split('/')
            .any(|part| part.is_empty() || part == "." || part == "..");
        if invalid {
            return Err(PathError::InvalidComponent(raw.to_owned()));
        }
        Ok(Self(trimmed.to_owned()))
    }

    #[must_use]
    pub const fn root() -> Self {
        Self(String::new())
    }

    #[must_use]
    pub fn as_str(&self) -> &str {
        &self.0
    }

    #[must_use]
    pub fn is_root(&self) -> bool {
        self.0.is_empty()
    }

    pub fn components(&self) -> impl Iterator<Item = &str> {
        self.0.split('/').filter(|part| !part.is_empty())
    }

    pub fn join(&self, relative: &str) -> Result<Self, PathError> {
        let tail = Self::new(relative)?;
        if self.is_root() {
            return Ok(tail);
        }
        if tail.is_root() {
            return Ok(self.clone());
        }
        Ok(Self(format!("{}/{}", self.0, tail.0)))
    }

    #[must_use]
    pub fn parent(&self) -> Option<Self> {
        if self.is_root() {
            return None;
        }
        match self.0.rsplit_once('/') {
            Some((parent, _)) => Some(Self(parent.to_owned())),
            None => Some(Self::root()),
        }
    }

    #[must_use]
    pub fn file_name(&self) -> &str {
        self.0.rsplit('/').next().unwrap_or_default()
    }

    #[must_use]
    pub fn file_stem(&self) -> &str {
        let name = self.file_name();
        match name.rsplit_once('.') {
            Some((stem, _)) if !stem.is_empty() => stem,
            _ => name,
        }
    }

    #[must_use]
    pub fn extension(&self) -> Option<&str> {
        let name = self.file_name();
        match name.rsplit_once('.') {
            Some((stem, extension)) if !stem.is_empty() => Some(extension),
            _ => None,
        }
    }

    #[must_use]
    pub fn has_extension(&self, extension: &str) -> bool {
        self.extension()
            .is_some_and(|found| found.eq_ignore_ascii_case(extension))
    }

    #[must_use]
    pub fn is_hidden(&self) -> bool {
        self.components().any(|part| part.starts_with('.'))
    }

    #[must_use]
    pub fn starts_with(&self, prefix: &Self) -> bool {
        prefix.is_root()
            || self.0 == prefix.0
            || self
                .0
                .strip_prefix(&prefix.0)
                .is_some_and(|rest| rest.starts_with('/'))
    }

    #[must_use]
    pub fn strip_prefix(&self, prefix: &Self) -> Option<Self> {
        if !self.starts_with(prefix) {
            return None;
        }
        let rest = self.0.get(prefix.0.len()..).unwrap_or_default();
        Some(Self(rest.trim_start_matches('/').to_owned()))
    }

    pub fn with_file_name(&self, name: &str) -> Result<Self, PathError> {
        if self.is_root() {
            return Err(PathError::Root);
        }
        if name.contains('/') {
            return Err(PathError::InvalidComponent(name.to_owned()));
        }
        self.parent().unwrap_or_default().join(name)
    }
}

impl fmt::Display for RelativePath {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.0)
    }
}

impl FromStr for RelativePath {
    type Err = PathError;

    fn from_str(raw: &str) -> Result<Self, Self::Err> {
        Self::new(raw)
    }
}

impl TryFrom<String> for RelativePath {
    type Error = PathError;

    fn try_from(raw: String) -> Result<Self, Self::Error> {
        Self::new(&raw)
    }
}

impl From<RelativePath> for String {
    fn from(path: RelativePath) -> Self {
        path.0
    }
}

impl AsRef<str> for RelativePath {
    fn as_ref(&self) -> &str {
        &self.0
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn path(raw: &str) -> RelativePath {
        RelativePath::new(raw).expect("the test path is valid")
    }

    #[test]
    fn normalizes_surrounding_slashes() {
        assert_eq!(path("/notes/a.md/").as_str(), "notes/a.md");
        assert!(path("").is_root());
        assert!(path("/").is_root());
    }

    #[test]
    fn rejects_escaping_and_ambiguous_paths() {
        assert!(matches!(
            RelativePath::new("a/../b"),
            Err(PathError::InvalidComponent(_))
        ));
        assert!(matches!(
            RelativePath::new("./a"),
            Err(PathError::InvalidComponent(_))
        ));
        assert!(matches!(
            RelativePath::new("a//b"),
            Err(PathError::InvalidComponent(_))
        ));
        assert!(matches!(
            RelativePath::new("a\\b"),
            Err(PathError::InvalidCharacter(_))
        ));
        assert!(matches!(
            RelativePath::new("a\0b"),
            Err(PathError::InvalidCharacter(_))
        ));
    }

    #[test]
    fn splits_names_and_extensions() {
        let note = path("notes/Plan.v2.md");
        assert_eq!(note.file_name(), "Plan.v2.md");
        assert_eq!(note.file_stem(), "Plan.v2");
        assert_eq!(note.extension(), Some("md"));
        assert!(note.has_extension("MD"));
        assert_eq!(path(".gitignore").extension(), None);
        assert_eq!(path("README").extension(), None);
        assert_eq!(note.parent(), Some(path("notes")));
        assert_eq!(path("a.md").parent(), Some(RelativePath::root()));
        assert_eq!(RelativePath::root().parent(), None);
    }

    #[test]
    fn joins_and_strips_prefixes_by_component() {
        let folder = path("notes");
        assert_eq!(
            folder.join("sub/a.md").expect("valid join"),
            path("notes/sub/a.md")
        );
        assert_eq!(
            RelativePath::root().join("a.md").expect("valid join"),
            path("a.md")
        );
        assert!(matches!(
            folder.join("../x"),
            Err(PathError::InvalidComponent(_))
        ));
        assert!(path("notes/a.md").starts_with(&folder));
        assert!(!path("notesbook/a.md").starts_with(&folder));
        assert_eq!(
            path("notes/sub/a.md").strip_prefix(&folder),
            Some(path("sub/a.md"))
        );
        assert_eq!(path("other/a.md").strip_prefix(&folder), None);
    }

    #[test]
    fn detects_hidden_components() {
        assert!(path(".margin/index.sqlite").is_hidden());
        assert!(path("notes/.draft/a.md").is_hidden());
        assert!(!path("notes/a.md").is_hidden());
    }

    #[test]
    fn renames_within_the_same_folder() {
        assert_eq!(
            path("notes/a.md")
                .with_file_name("b.md")
                .expect("valid name"),
            path("notes/b.md")
        );
        assert!(matches!(
            path("notes/a.md").with_file_name("x/b.md"),
            Err(PathError::InvalidComponent(_))
        ));
        assert_eq!(
            RelativePath::root().with_file_name("a"),
            Err(PathError::Root)
        );
    }

    #[test]
    fn deserialization_validates() {
        let parsed: Result<RelativePath, _> = serde_json::from_str("\"a/../b\"");
        let error = parsed.expect_err("an escaping path is rejected");
        assert!(
            error.to_string().contains("component"),
            "unexpected error: {error}"
        );
        let ok: RelativePath = serde_json::from_str("\"a/b\"").expect("a valid path deserializes");
        assert_eq!(ok, path("a/b"));
    }
}
