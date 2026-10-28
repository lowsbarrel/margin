use margin_kernel::StoreError;
use margin_sync::BlobStore;

pub async fn blob_store_conformance(store: &dyn BlobStore) {
    let simple = "conformance/simple.bin";
    let nested = "conformance/nested/b.bin";
    let missing = "conformance/missing.bin";

    store
        .put(simple, b"alpha".to_vec())
        .await
        .expect("put stores an object");
    store
        .put(nested, b"beta".to_vec())
        .await
        .expect("put stores a nested object");
    assert_eq!(
        store.get(simple).await.expect("get returns the object"),
        b"alpha"
    );
    assert_eq!(
        store
            .head(simple)
            .await
            .expect("head returns the body length"),
        Some(5)
    );
    assert!(matches!(
        store.get(missing).await,
        Err(StoreError::NotFound(_))
    ));
    assert_eq!(
        store.head(missing).await.expect("head reports absence"),
        None
    );

    let mut listed = store
        .list_prefix("conformance/")
        .await
        .expect("list returns keys");
    listed.sort();
    assert!(
        listed.contains(&simple.to_owned()),
        "listing omits {simple}: {listed:?}"
    );
    assert!(
        listed.contains(&nested.to_owned()),
        "listing omits {nested}: {listed:?}"
    );

    store
        .put(simple, b"ALPHA".to_vec())
        .await
        .expect("put overwrites");
    assert_eq!(
        store.get(simple).await.expect("get returns the new body"),
        b"ALPHA"
    );

    store
        .delete(simple)
        .await
        .expect("delete removes an object");
    assert!(matches!(
        store.get(simple).await,
        Err(StoreError::NotFound(_))
    ));
    store
        .delete(simple)
        .await
        .expect("deleting a missing object is not an error");
    store
        .delete(nested)
        .await
        .expect("delete removes a nested object");
}
