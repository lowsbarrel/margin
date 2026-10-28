use crate::{VaultId, VaultKey};

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct DerivedVault {
    pub id: VaultId,
    pub key: VaultKey,
}
