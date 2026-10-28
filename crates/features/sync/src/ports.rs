use margin_kernel::{BoxFuture, RelativePath, StoreError};

pub trait BlobStore: Send + Sync {
    fn put<'a>(&'a self, key: &'a str, body: Vec<u8>) -> BoxFuture<'a, Result<(), StoreError>>;

    fn get<'a>(&'a self, key: &'a str) -> BoxFuture<'a, Result<Vec<u8>, StoreError>>;

    fn head<'a>(&'a self, key: &'a str) -> BoxFuture<'a, Result<Option<u64>, StoreError>>;

    fn delete<'a>(&'a self, key: &'a str) -> BoxFuture<'a, Result<(), StoreError>>;

    fn list_prefix<'a>(&'a self, prefix: &'a str)
    -> BoxFuture<'a, Result<Vec<String>, StoreError>>;
}

pub trait SnapshotStore: Send + Sync {
    fn save(&self, path: &RelativePath, contents: &[u8]) -> Result<(), StoreError>;
}
