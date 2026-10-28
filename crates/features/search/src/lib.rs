mod domain;
mod error;
mod ports;
mod service;

pub use domain::{
    Backlink, FuzzyEntry, IndexedNote, IndexedNoteMeta, SearchHit, SearchMode, TagInfo,
    backlink_target, build_match_for, build_match_query, extract_tags, filter_files,
    parse_wiki_links,
};
pub use error::Error;
pub use ports::SearchIndex;
pub use service::SearchService;
