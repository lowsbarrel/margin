use std::fmt;

use serde::{Deserialize, Serialize};

#[derive(Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(transparent)]
pub struct Secret<T>(T);

impl<T> Secret<T> {
    #[must_use]
    pub const fn new(value: T) -> Self {
        Self(value)
    }

    #[must_use]
    pub const fn expose(&self) -> &T {
        &self.0
    }

    #[must_use]
    pub fn into_exposed(self) -> T {
        self.0
    }
}

impl<T> fmt::Debug for Secret<T> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("Secret(redacted)")
    }
}

impl<T> From<T> for Secret<T> {
    fn from(value: T) -> Self {
        Self(value)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn debug_never_prints_the_value() {
        let secret = Secret::new(String::from("abandon ability able"));
        assert_eq!(format!("{secret:?}"), "Secret(redacted)");
        assert_eq!(secret.expose(), "abandon ability able");
    }

    #[test]
    fn serializes_as_the_plain_value() {
        let secret = Secret::new(String::from("key"));
        let json = serde_json::to_string(&secret).expect("a string serializes");
        assert_eq!(json, "\"key\"");
        let back: Secret<String> = serde_json::from_str(&json).expect("a string deserializes");
        assert_eq!(back, secret);
    }
}
