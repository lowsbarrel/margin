use std::sync::Arc;

use margin_settings::S3Config;
use margin_sync::SyncService;

pub trait SyncFactory: Send + Sync {
    fn build(&self, config: &S3Config) -> Arc<SyncService>;
}
