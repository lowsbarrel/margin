use std::collections::HashSet;
use std::ffi::OsStr;
use std::path::Path;

use margin_kernel::Secret;
use serde::{Deserialize, Serialize};

const DEFAULT_PROFILE_NAME: &str = "Vault";

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct VaultProfile {
    pub name: String,
    pub mnemonic: Secret<String>,
    pub vault_path: String,
}

#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct VaultProfiles {
    pub profiles: Vec<VaultProfile>,
    pub last_used: Option<String>,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct LegacySession {
    pub mnemonic: Secret<String>,
    pub vault_path: String,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct VaultProfileInfo {
    pub name: String,
    pub vault_path: String,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct VaultProfilesInfo {
    pub profiles: Vec<VaultProfileInfo>,
    pub last_used: Option<String>,
}

impl From<&VaultProfile> for VaultProfileInfo {
    fn from(profile: &VaultProfile) -> Self {
        Self {
            name: profile.name.clone(),
            vault_path: profile.vault_path.clone(),
        }
    }
}

impl VaultProfiles {
    #[must_use]
    pub fn normalised(self) -> Self {
        let mut seen = HashSet::new();
        let profiles = self
            .profiles
            .into_iter()
            .map(|mut profile| {
                profile.vault_path = normalise_slashes(&profile.vault_path);
                profile
            })
            .filter(|profile| seen.insert(profile.vault_path.clone()))
            .collect();
        Self {
            profiles,
            last_used: self.last_used.map(|path| normalise_slashes(&path)),
        }
    }

    #[must_use]
    pub fn session(&self) -> Option<VaultProfileInfo> {
        if let Some(last_used) = self.last_used.as_deref()
            && let Some(profile) = self
                .profiles
                .iter()
                .find(|profile| profile.vault_path == last_used)
        {
            return Some(VaultProfileInfo::from(profile));
        }
        self.profiles.first().map(VaultProfileInfo::from)
    }

    #[must_use]
    pub fn into_info(self) -> VaultProfilesInfo {
        VaultProfilesInfo {
            profiles: self.profiles.iter().map(VaultProfileInfo::from).collect(),
            last_used: self.last_used,
        }
    }
}

#[must_use]
pub fn normalise_slashes(path: &str) -> String {
    path.replace('\\', "/")
}

#[must_use]
pub fn profile_name_from_vault_path(vault_path: &str) -> String {
    Path::new(vault_path)
        .file_name()
        .and_then(OsStr::to_str)
        .unwrap_or(DEFAULT_PROFILE_NAME)
        .to_owned()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn profile(name: &str, vault_path: &str) -> VaultProfile {
        VaultProfile {
            name: name.to_owned(),
            mnemonic: Secret::new(String::from("mnemonic")),
            vault_path: vault_path.to_owned(),
        }
    }

    #[test]
    fn normalises_and_deduplicates_by_vault_path() {
        let registry = VaultProfiles {
            profiles: vec![
                profile("One", "C:\\Users\\test\\Vault"),
                profile("Duplicate", "C:/Users/test/Vault"),
                profile("Two", "D:/Work"),
            ],
            last_used: Some("C:\\Users\\test\\Vault".to_owned()),
        }
        .normalised();
        assert_eq!(registry.profiles.len(), 2);
        assert_eq!(registry.profiles[0].name, "One");
        assert_eq!(registry.profiles[0].vault_path, "C:/Users/test/Vault");
        assert_eq!(registry.last_used.as_deref(), Some("C:/Users/test/Vault"));
    }

    #[test]
    fn session_prefers_the_last_used_profile() {
        let registry = VaultProfiles {
            profiles: vec![profile("One", "a"), profile("Two", "b")],
            last_used: Some("b".to_owned()),
        };
        assert_eq!(
            registry
                .session()
                .expect("the last used profile resolves")
                .name,
            "Two"
        );
    }

    #[test]
    fn session_falls_back_to_the_first_profile() {
        let unknown = VaultProfiles {
            profiles: vec![profile("One", "a"), profile("Two", "b")],
            last_used: Some("missing".to_owned()),
        };
        assert_eq!(
            unknown
                .session()
                .expect("a profile is still available")
                .name,
            "One"
        );
        let empty = VaultProfiles::default();
        assert_eq!(empty.session(), None);
    }

    #[test]
    fn names_a_profile_after_its_folder() {
        assert_eq!(profile_name_from_vault_path("C:/Users/test/Vault"), "Vault");
        assert_eq!(profile_name_from_vault_path("/home/test/Notes/"), "Notes");
        assert_eq!(profile_name_from_vault_path(""), "Vault");
        assert_eq!(profile_name_from_vault_path("/"), "Vault");
    }

    #[test]
    fn serialises_the_release_registry_shape() {
        let registry = VaultProfiles {
            profiles: vec![VaultProfile {
                name: String::from("Notes"),
                mnemonic: Secret::new(String::from("word word")),
                vault_path: String::from("C:/Vault"),
            }],
            last_used: None,
        };
        let json = serde_json::to_string(&registry).expect("a registry serializes");
        assert_eq!(
            json,
            r#"{"profiles":[{"name":"Notes","mnemonic":"word word","vault_path":"C:/Vault"}],"last_used":null}"#
        );
        let back: VaultProfiles = serde_json::from_str(&json).expect("a registry deserializes");
        assert_eq!(back, registry);
    }

    #[test]
    fn serialises_the_release_legacy_session_shape() {
        let legacy = LegacySession {
            mnemonic: Secret::new(String::from("word word")),
            vault_path: String::from("C:/Vault"),
        };
        let json = serde_json::to_string(&legacy).expect("a legacy session serializes");
        assert_eq!(json, r#"{"mnemonic":"word word","vault_path":"C:/Vault"}"#);
    }

    #[test]
    fn debug_never_prints_a_mnemonic() {
        assert_eq!(
            format!("{:?}", profile("One", "a")),
            "VaultProfile { name: \"One\", mnemonic: Secret(redacted), vault_path: \"a\" }"
        );
    }
}
