use std::sync::Arc;

use margin_kernel::{FileSystem, HostFiles};
use margin_test_support::{MemoryFileSystem, MemoryHostFiles, check_file_system, check_host_files};

#[test]
fn memory_file_system_passes_the_conformance_suite() {
    check_file_system(|| {
        let filesystem: Arc<dyn FileSystem> = Arc::new(MemoryFileSystem::new());
        filesystem
    });
}

#[test]
fn memory_host_files_passes_the_conformance_suite() {
    let directory = tempfile::tempdir().expect("a temporary directory");
    check_host_files(
        || {
            let host: Arc<dyn HostFiles> = Arc::new(MemoryHostFiles::new());
            host
        },
        directory.path(),
    );
}
