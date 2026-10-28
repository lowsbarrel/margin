use margin_test_support::{MemorySearchIndex, search_index_conformance};

#[test]
fn the_in_memory_index_passes_the_search_index_conformance_suite() {
    search_index_conformance(&MemorySearchIndex::new());
}
