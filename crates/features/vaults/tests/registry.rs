use std::sync::Arc;

use margin_crypto::{VaultKey, decrypt, encrypt};
use margin_kernel::{FileSystem, Secret};
use margin_test_support::MemoryFileSystem;
use margin_vaults::VaultService;

const FIXTURE_MNEMONIC: &str =
    "abandon abandon abandon abandon abandon abandon abandon abandon abandon abandon abandon about";
const FIXTURE_VAULT_ID: &str = "209947b258e347c7f68ea97be6a866a7771d2363696ff1fa585d6d28ae45e1af";
const FIXTURE_DEVICE_KEY: [u8; 32] = [
    0, 1, 2, 3, 4, 5, 6, 7, 8, 9, 10, 11, 12, 13, 14, 15, 16, 17, 18, 19, 20, 21, 22, 23, 24, 25,
    26, 27, 28, 29, 30, 31,
];
const FIXTURE_REGISTRY_JSON: &str = r#"{"profiles":[{"name":"Notes","mnemonic":"abandon abandon abandon abandon abandon abandon abandon abandon abandon abandon abandon about","vault_path":"C:/Users/test/Vault"},{"name":"Work","mnemonic":"abandon abandon abandon abandon abandon abandon abandon abandon abandon abandon abandon about","vault_path":"D:/Work/Archive"}],"last_used":"D:/Work/Archive"}"#;
const MIGRATED_REGISTRY_JSON: &str = r#"{"profiles":[{"name":"Vault","mnemonic":"abandon abandon abandon abandon abandon abandon abandon abandon abandon abandon abandon about","vault_path":"C:/Users/test/Vault"}],"last_used":"C:/Users/test/Vault"}"#;
const TRUNCATED_DEVICE_KEY: [u8; 5] = [0; 5];

const REGISTRY_BLOB: &[u8] = &[
    160, 161, 162, 163, 164, 165, 166, 167, 168, 169, 170, 171, 151, 7, 39, 4, 97, 117, 8, 43, 43,
    34, 143, 177, 138, 214, 151, 71, 20, 45, 90, 229, 91, 77, 107, 197, 46, 212, 62, 154, 254, 44,
    142, 202, 151, 75, 25, 217, 152, 100, 55, 165, 9, 157, 59, 97, 10, 214, 233, 195, 91, 224, 213,
    245, 180, 213, 209, 73, 218, 117, 228, 78, 190, 192, 59, 67, 157, 65, 37, 200, 251, 112, 123,
    186, 95, 33, 91, 104, 209, 69, 250, 19, 71, 36, 101, 106, 84, 54, 136, 147, 4, 38, 71, 214, 68,
    73, 60, 179, 108, 138, 221, 128, 104, 61, 33, 185, 238, 11, 85, 182, 71, 91, 208, 59, 0, 130,
    14, 121, 133, 159, 13, 99, 215, 246, 236, 93, 231, 113, 23, 103, 219, 183, 233, 16, 73, 143,
    155, 76, 190, 126, 209, 80, 135, 175, 178, 161, 52, 54, 68, 144, 170, 169, 96, 10, 160, 61, 48,
    75, 201, 251, 15, 90, 223, 16, 130, 40, 245, 196, 251, 60, 117, 218, 85, 248, 242, 40, 227,
    133, 154, 236, 199, 91, 253, 212, 145, 179, 183, 192, 175, 68, 88, 15, 222, 179, 126, 157, 101,
    137, 202, 135, 170, 76, 220, 33, 133, 5, 30, 28, 93, 203, 102, 115, 5, 229, 231, 114, 72, 218,
    45, 67, 19, 249, 208, 226, 11, 225, 15, 196, 104, 248, 71, 13, 176, 142, 132, 40, 129, 143,
    213, 223, 163, 170, 188, 2, 13, 150, 180, 106, 100, 208, 86, 194, 63, 151, 30, 98, 137, 83,
    185, 83, 88, 185, 50, 31, 83, 152, 196, 116, 181, 132, 17, 189, 48, 111, 205, 196, 161, 88,
    182, 55, 35, 125, 21, 54, 149, 193, 71, 205, 5, 213, 168, 251, 144, 89, 45, 73, 131, 113, 33,
    25, 202, 200, 73, 185, 62, 177, 180, 212, 181, 128, 183, 68, 95, 84, 72, 220, 235, 102, 198,
    125, 244, 148, 244, 241, 10, 200, 31, 136, 190, 187, 34, 187, 15, 58, 91, 17, 193, 192, 86,
    150, 124, 68, 73, 136, 228, 221, 138, 123, 32, 164, 182, 238, 107, 246, 251, 172, 184, 21, 218,
    5, 115, 134, 75, 7, 134, 252, 121, 102, 7, 213, 25, 170, 253, 39, 106,
];
const LEGACY_BLOB: &[u8] = &[
    160, 161, 162, 163, 164, 165, 166, 167, 168, 169, 170, 171, 181, 43, 80, 141, 185, 39, 113, 3,
    232, 196, 180, 227, 43, 134, 81, 18, 82, 65, 240, 57, 192, 50, 197, 1, 125, 157, 95, 222, 21,
    34, 70, 157, 36, 69, 255, 160, 153, 35, 32, 106, 175, 128, 98, 33, 241, 144, 47, 110, 155, 213,
    232, 69, 10, 20, 6, 241, 195, 34, 93, 62, 13, 214, 129, 217, 142, 41, 91, 93, 18, 104, 124, 14,
    118, 28, 68, 55, 133, 253, 211, 185, 73, 141, 146, 160, 171, 140, 11, 182, 110, 12, 234, 121,
    198, 170, 65, 196, 136, 13, 60, 108, 102, 109, 182, 138, 249, 252, 107, 130, 58, 219, 242, 158,
    11, 232, 229, 237, 86, 56, 135, 142, 134, 104, 49, 18, 147, 141, 220, 177, 144, 169, 221, 157,
    31, 70, 185, 156, 3, 178, 135, 86, 182, 96, 135, 15, 142, 241, 29, 68, 169, 188, 10, 245, 7,
    221, 83, 185, 85, 141, 158,
];

fn fixture_key() -> VaultKey {
    VaultKey::from_bytes(FIXTURE_DEVICE_KEY)
}

fn service_over(entries: &[(&str, &[u8])]) -> (Arc<MemoryFileSystem>, VaultService) {
    let memory = Arc::new(MemoryFileSystem::new());
    for (path, contents) in entries {
        memory.insert(path, contents);
    }
    let file_system: Arc<dyn FileSystem> = Arc::<MemoryFileSystem>::clone(&memory);
    (memory, VaultService::new(file_system))
}

fn fixture_service(entries: &[(&str, &[u8])]) -> (Arc<MemoryFileSystem>, VaultService) {
    let mut all = vec![("device.key", &FIXTURE_DEVICE_KEY[..])];
    all.extend_from_slice(entries);
    service_over(&all)
}

#[test]
fn reads_a_registry_written_by_the_release() {
    let (_memory, service) = fixture_service(&[("profiles.enc", REGISTRY_BLOB)]);
    let info = service.load_profiles().expect("the release registry opens");
    assert_eq!(info.profiles.len(), 2);
    assert_eq!(info.profiles[0].name, "Notes");
    assert_eq!(info.profiles[0].vault_path, "C:/Users/test/Vault");
    assert_eq!(info.profiles[1].name, "Work");
    assert_eq!(info.profiles[1].vault_path, "D:/Work/Archive");
    assert_eq!(info.last_used.as_deref(), Some("D:/Work/Archive"));

    let session = service
        .load_session()
        .expect("the session reads")
        .expect("a profile is remembered");
    assert_eq!(session.name, "Work");
    assert_eq!(session.vault_path, "D:/Work/Archive");

    let vault = service
        .unlock("D:/Work/Archive")
        .expect("the stored mnemonic derives");
    assert_eq!(vault.id.as_str(), FIXTURE_VAULT_ID);
}

#[test]
fn writes_the_release_registry_format() {
    let (memory, service) = fixture_service(&[]);
    let mnemonic = Secret::new(String::from(FIXTURE_MNEMONIC));
    service
        .save_profile("Notes", "C:/Users/test/Vault", &mnemonic)
        .expect("the profile saves");
    service
        .save_profile("Work", "D:/Work/Archive", &mnemonic)
        .expect("the profile saves");

    let written = memory
        .contents("profiles.enc")
        .expect("the registry is written");
    let plaintext = decrypt(&fixture_key(), &written).expect("the written registry decrypts");
    assert_eq!(plaintext, FIXTURE_REGISTRY_JSON.as_bytes());
}

#[test]
fn migrates_a_legacy_session_file() {
    let (memory, service) = fixture_service(&[("session.enc", LEGACY_BLOB)]);
    let info = service
        .load_profiles()
        .expect("the legacy session migrates");
    assert_eq!(info.profiles.len(), 1);
    assert_eq!(info.profiles[0].name, "Vault");
    assert_eq!(info.profiles[0].vault_path, "C:/Users/test/Vault");
    assert_eq!(info.last_used.as_deref(), Some("C:/Users/test/Vault"));
    assert_eq!(memory.contents("session.enc"), None);

    let written = memory
        .contents("profiles.enc")
        .expect("the migrated registry is written");
    let plaintext = decrypt(&fixture_key(), &written).expect("the migrated registry decrypts");
    assert_eq!(plaintext, MIGRATED_REGISTRY_JSON.as_bytes());

    let vault = service
        .unlock("C:/Users/test/Vault")
        .expect("the migrated mnemonic derives");
    assert_eq!(vault.id.as_str(), FIXTURE_VAULT_ID);
}

#[test]
fn keeps_a_legacy_session_whose_json_does_not_fit() {
    let blob = encrypt(&fixture_key(), b"{\"mnemonic\":1}");
    let (memory, service) = fixture_service(&[("session.enc", &blob)]);
    let info = service
        .load_profiles()
        .expect("an unfitting legacy session is not a failure");
    assert!(info.profiles.is_empty());
    assert_eq!(
        memory.contents("session.enc").as_deref(),
        Some(blob.as_slice())
    );
}

#[test]
fn quarantines_an_unreadable_registry() {
    let (memory, service) = fixture_service(&[("profiles.enc", b"not a blob")]);
    let info = service
        .load_profiles()
        .expect("an unreadable registry is not a failure");
    assert!(info.profiles.is_empty());
    assert_eq!(info.last_used, None);
    assert_eq!(memory.contents("profiles.enc"), None);
    assert_eq!(
        memory.contents("profiles.enc.bad").as_deref(),
        Some(&b"not a blob"[..])
    );
}

#[test]
fn quarantines_a_registry_whose_json_does_not_fit() {
    let blob = encrypt(&fixture_key(), b"{\"profiles\":\"wrong\"}");
    let (memory, service) = fixture_service(&[("profiles.enc", &blob)]);
    let info = service
        .load_profiles()
        .expect("an unreadable registry is not a failure");
    assert!(info.profiles.is_empty());
    assert_eq!(memory.contents("profiles.enc"), None);
    assert_eq!(
        memory.contents("profiles.enc.bad").as_deref(),
        Some(blob.as_slice())
    );
}

#[test]
fn keeps_an_unreadable_legacy_session() {
    let (memory, service) = fixture_service(&[("session.enc", b"not a blob")]);
    let info = service
        .load_profiles()
        .expect("an unreadable legacy session is not a failure");
    assert!(info.profiles.is_empty());
    assert_eq!(
        memory.contents("session.enc").as_deref(),
        Some(&b"not a blob"[..])
    );
}

#[test]
fn normalises_and_deduplicates_a_stored_registry() {
    let json = r#"{"profiles":[{"name":"One","mnemonic":"mnemonic","vault_path":"C:\\Users\\test\\Vault"},{"name":"Duplicate","mnemonic":"mnemonic","vault_path":"C:/Users/test/Vault"}],"last_used":"C:\\Users\\test\\Vault"}"#;
    let blob = encrypt(&fixture_key(), json.as_bytes());
    let (_memory, service) = fixture_service(&[("profiles.enc", &blob)]);

    let info = service.load_profiles().expect("the registry opens");
    assert_eq!(info.profiles.len(), 1);
    assert_eq!(info.profiles[0].name, "One");
    assert_eq!(info.profiles[0].vault_path, "C:/Users/test/Vault");
    assert_eq!(info.last_used.as_deref(), Some("C:/Users/test/Vault"));
}

#[test]
fn replaces_a_device_key_of_the_wrong_length() {
    let (memory, service) = service_over(&[
        ("device.key", &TRUNCATED_DEVICE_KEY[..]),
        ("profiles.enc", REGISTRY_BLOB),
    ]);
    let info = service
        .load_profiles()
        .expect("a broken device key is replaced");
    assert!(info.profiles.is_empty());
    assert_eq!(memory.contents("device.key").map(|key| key.len()), Some(32));
    assert_eq!(memory.contents("profiles.enc"), None);
}
