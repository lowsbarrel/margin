use std::sync::Arc;

use margin_fs::StdHostFiles;
use margin_kernel::HostFiles;
use margin_test_support::{MemoryHostFiles, check_host_files};

#[test]
fn std_host_files_passes_the_conformance_suite() {
    let directory = tempfile::tempdir().expect("a temporary directory");
    check_host_files(real_host_files, directory.path());
}

#[test]
fn the_suite_also_holds_for_the_in_memory_fake() {
    let directory = tempfile::tempdir().expect("a temporary directory");
    check_host_files(memory_host_files, directory.path());
}

fn real_host_files() -> Arc<dyn HostFiles> {
    Arc::new(StdHostFiles::new())
}

fn memory_host_files() -> Arc<dyn HostFiles> {
    Arc::new(MemoryHostFiles::new())
}
