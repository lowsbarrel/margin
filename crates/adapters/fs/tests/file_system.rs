use std::fs;
use std::path::Path;
use std::sync::Arc;
use std::sync::atomic::{AtomicUsize, Ordering};

use margin_fs::StdFileSystem;
use margin_kernel::FileSystem;
use margin_test_support::check_file_system;

#[test]
fn std_file_system_passes_the_conformance_suite() {
    let parent = tempfile::tempdir().expect("a temporary directory");
    let counter = AtomicUsize::new(0);
    check_file_system(|| rooted_vault(parent.path(), &counter));
}

fn rooted_vault(parent: &Path, counter: &AtomicUsize) -> Arc<dyn FileSystem> {
    let root = parent.join(format!("vault-{}", counter.fetch_add(1, Ordering::Relaxed)));
    fs::create_dir_all(&root).expect("a fresh root");
    Arc::new(StdFileSystem::new(root))
}
