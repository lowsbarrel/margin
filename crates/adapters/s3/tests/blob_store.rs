use margin_kernel::StoreError;
use margin_s3::{S3BlobStore, S3Config};
use margin_sync::BlobStore;
use margin_test_support::{MemoryBlobStore, S3Stub, blob_store_conformance};

fn store(stub: &S3Stub) -> S3BlobStore {
    S3BlobStore::new(S3Config::new(
        stub.endpoint(),
        stub.bucket(),
        stub.region(),
        stub.access_key(),
        stub.secret_key(),
    ))
}

#[tokio::test]
async fn memory_blob_store_passes_the_conformance_suite() {
    blob_store_conformance(&MemoryBlobStore::new()).await;
}

#[tokio::test]
async fn s3_blob_store_passes_the_conformance_suite_against_the_stub() {
    let stub = S3Stub::start();
    blob_store_conformance(&store(&stub)).await;
    assert_eq!(stub.object_count(), 0);
}

#[tokio::test]
async fn listing_returns_only_the_requested_prefix() {
    let stub = S3Stub::start();
    let store = store(&stub);
    store
        .put("a/one.enc", b"1".to_vec())
        .await
        .expect("put stores the first object");
    store
        .put("a/two.enc", b"22".to_vec())
        .await
        .expect("put stores the second object");
    store
        .put("b/three.enc", b"333".to_vec())
        .await
        .expect("put stores the third object");

    let mut keys = store.list_prefix("a/").await.expect("list returns keys");
    keys.sort();
    assert_eq!(keys, vec!["a/one.enc".to_owned(), "a/two.enc".to_owned()]);
    assert_eq!(
        store
            .list_prefix("")
            .await
            .expect("list returns keys")
            .len(),
        3
    );
}

#[tokio::test]
async fn head_reports_the_stored_size_and_absence() {
    let stub = S3Stub::start();
    let store = store(&stub);
    store
        .put("size.enc", b"0123456789".to_vec())
        .await
        .expect("put stores the object");
    assert_eq!(
        store.head("size.enc").await.expect("head succeeds"),
        Some(10)
    );
    assert_eq!(
        store.head("missing.enc").await.expect("head succeeds"),
        None
    );
    assert!(matches!(
        store.get("missing.enc").await,
        Err(StoreError::NotFound(_))
    ));
}

#[tokio::test]
async fn a_utf8_object_round_trips() {
    let stub = S3Stub::start();
    let store = store(&stub);
    let body = "café ሴ margin".as_bytes().to_vec();
    store
        .put("nested/dir/utf8.enc", body.clone())
        .await
        .expect("put stores a nested key");
    assert_eq!(
        store
            .get("nested/dir/utf8.enc")
            .await
            .expect("get returns the object"),
        body
    );
}

#[test]
fn config_debug_redacts_credentials() {
    let config = S3Config::new(
        "https://example.invalid",
        "bucket",
        "region-1",
        "access-key",
        "secret-key",
    );
    let text = format!("{config:?}");
    assert!(!text.contains("access-key"));
    assert!(!text.contains("secret-key"));
    assert!(text.contains("https://example.invalid"));
}
