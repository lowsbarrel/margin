use margin_crypto::{DerivedVault, derive_vault};
use margin_kernel::Secret;
use thiserror::Error;

#[derive(Clone, Copy, Debug, Error, PartialEq, Eq)]
pub(crate) enum PassphraseError {
    #[error("the passphrase is empty")]
    Empty,
    #[error("the passphrase is not a valid BIP-39 mnemonic")]
    Invalid,
}

impl PassphraseError {
    pub(crate) const fn message_key(self) -> &'static str {
        match self {
            Self::Empty => "login_error_no_mnemonic",
            Self::Invalid => "login_error_invalid_passphrase",
        }
    }
}

pub(crate) fn normalise(text: &str) -> Secret<String> {
    Secret::new(
        text.split_whitespace()
            .map(str::to_lowercase)
            .collect::<Vec<_>>()
            .join(" "),
    )
}

pub(crate) fn check_not_empty(text: &str) -> Result<(), PassphraseError> {
    if text.trim().is_empty() {
        Err(PassphraseError::Empty)
    } else {
        Ok(())
    }
}

pub(crate) fn derive(text: &str) -> Result<(Secret<String>, DerivedVault), PassphraseError> {
    check_not_empty(text)?;
    let mnemonic = normalise(text);
    let vault = derive_vault(mnemonic.expose())
        .ok()
        .ok_or(PassphraseError::Invalid)?;
    Ok((mnemonic, vault))
}

#[cfg(test)]
mod tests {
    use super::*;

    const WORDS: &str = "abandon abandon abandon abandon abandon abandon abandon abandon abandon abandon abandon about";

    #[test]
    fn accepts_pasted_words_with_stray_spacing_and_case() {
        let pasted = format!("  {}\n", WORDS.replace(' ', " \n ").to_uppercase());
        let (mnemonic, vault) = derive(&pasted).expect("the words are a valid mnemonic");
        assert_eq!(mnemonic.expose(), WORDS);
        assert_eq!(vault, derive_vault(WORDS).expect("the fixture is valid"));
    }

    #[test]
    fn reports_an_empty_passphrase() {
        assert_eq!(derive(" \n\t").map(|_| ()), Err(PassphraseError::Empty));
    }

    #[test]
    fn rejects_words_that_fail_the_checksum_or_list() {
        let wrong_checksum = WORDS.replace("about", "abandon");
        assert_eq!(
            derive(&wrong_checksum).map(|_| ()),
            Err(PassphraseError::Invalid)
        );
        assert_eq!(
            derive("these are not twelve real words").map(|_| ()),
            Err(PassphraseError::Invalid)
        );
    }
}
