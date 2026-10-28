use std::path::PathBuf;

use margin_crypto::DerivedVault;

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum OnboardingEvent {
    Unlocked {
        name: String,
        root: PathBuf,
        vault: DerivedVault,
    },
}
