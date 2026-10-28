use std::collections::{BTreeMap, VecDeque};

use margin_kernel::{BoxFuture, StoreError};
use margin_sync::BlobStore;
use parking_lot::Mutex;

#[derive(Debug, Default)]
pub struct MemoryBlobStore {
    objects: Mutex<BTreeMap<String, Vec<u8>>>,
    failures: Mutex<VecDeque<StoreError>>,
}

impl MemoryBlobStore {
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    pub fn insert(&self, key: &str, body: &[u8]) {
        self.objects.lock().insert(key.to_owned(), body.to_vec());
    }

    pub fn fail_next(&self, error: StoreError) {
        self.failures.lock().push_back(error);
    }

    #[must_use]
    pub fn keys(&self) -> Vec<String> {
        self.objects.lock().keys().cloned().collect()
    }

    #[must_use]
    pub fn len(&self) -> usize {
        self.objects.lock().len()
    }

    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.objects.lock().is_empty()
    }

    fn take_failure(&self) -> Result<(), StoreError> {
        self.failures.lock().pop_front().map_or(Ok(()), Err)
    }
}

impl BlobStore for MemoryBlobStore {
    fn put<'a>(&'a self, key: &'a str, body: Vec<u8>) -> BoxFuture<'a, Result<(), StoreError>> {
        Box::pin(async move {
            self.take_failure()?;
            self.objects.lock().insert(key.to_owned(), body);
            Ok(())
        })
    }

    fn get<'a>(&'a self, key: &'a str) -> BoxFuture<'a, Result<Vec<u8>, StoreError>> {
        Box::pin(async move {
            self.take_failure()?;
            self.objects
                .lock()
                .get(key)
                .cloned()
                .ok_or_else(|| StoreError::NotFound(key.to_owned()))
        })
    }

    fn head<'a>(&'a self, key: &'a str) -> BoxFuture<'a, Result<Option<u64>, StoreError>> {
        Box::pin(async move {
            self.take_failure()?;
            Ok(self
                .objects
                .lock()
                .get(key)
                .map(|body| u64::try_from(body.len()).unwrap_or(u64::MAX)))
        })
    }

    fn delete<'a>(&'a self, key: &'a str) -> BoxFuture<'a, Result<(), StoreError>> {
        Box::pin(async move {
            self.take_failure()?;
            self.objects.lock().remove(key);
            Ok(())
        })
    }

    fn list_prefix<'a>(
        &'a self,
        prefix: &'a str,
    ) -> BoxFuture<'a, Result<Vec<String>, StoreError>> {
        Box::pin(async move {
            self.take_failure()?;
            Ok(self
                .objects
                .lock()
                .keys()
                .filter(|key| key.starts_with(prefix))
                .cloned()
                .collect())
        })
    }
}
