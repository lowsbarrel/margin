use std::sync::Arc;

use margin_s3::{S3BlobStore, S3Config};
use margin_test_support::{S3Stub, sync_round_trip};

#[tokio::test]
async fn two_vaults_round_trip_through_the_s3_adapter() {
    let stub = S3Stub::start();
    let store = S3BlobStore::new(S3Config::new(
        stub.endpoint(),
        stub.bucket(),
        stub.region(),
        stub.access_key(),
        stub.secret_key(),
    ));
    sync_round_trip(Arc::new(store)).await;
}
