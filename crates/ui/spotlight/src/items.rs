use margin_files::FsEntry;
use margin_kernel::RelativePath;
use margin_search::{SearchHit, TagInfo};

use crate::query::Query;

#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) enum Item {
    Name { path: RelativePath, name: String },
    Content(SearchHit),
    Tag { tag: String, count: usize },
    TagFile(RelativePath),
}

impl Item {
    const fn group(&self) -> Option<Group> {
        match self {
            Self::Name { .. } => Some(Group::Names),
            Self::Content(_) => Some(Group::Contents),
            Self::Tag { .. } => Some(Group::Tags),
            Self::TagFile(_) => None,
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum Group {
    Names,
    Contents,
    Tags,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum Row {
    Header { group: Group, count: usize },
    Item(usize),
}

pub(crate) fn build_items(
    query: &Query,
    selected_tag: Option<&str>,
    tags: &[TagInfo],
    names: &[FsEntry],
    hits: &[SearchHit],
) -> Vec<Item> {
    match query {
        Query::Ask(_) => Vec::new(),
        Query::Tags { filter } => match selected_tag {
            Some(selected) => {
                let wanted = selected.to_lowercase();
                tags.iter()
                    .find(|info| info.tag.to_lowercase() == wanted)
                    .map(|info| info.files.iter().cloned().map(Item::TagFile).collect())
                    .unwrap_or_default()
            }
            None => tags
                .iter()
                .filter(|info| filter.is_empty() || info.tag.to_lowercase().contains(filter))
                .map(|info| Item::Tag {
                    tag: info.tag.clone(),
                    count: info.count,
                })
                .collect(),
        },
        Query::Empty | Query::Search(_) => {
            let mut items: Vec<Item> = names
                .iter()
                .map(|entry| Item::Name {
                    path: entry.path.clone(),
                    name: entry.name.clone(),
                })
                .collect();
            for hit in hits {
                if !names.iter().any(|entry| entry.path == hit.path) {
                    items.push(Item::Content(hit.clone()));
                }
            }
            items
        }
    }
}

pub(crate) fn rows(items: &[Item]) -> Vec<Row> {
    let mut rows = Vec::with_capacity(items.len() + 2);
    let mut previous = None;
    for (index, item) in items.iter().enumerate() {
        let group = item.group();
        if let Some(group) = group
            && previous != Some(group)
        {
            let count = items
                .iter()
                .filter(|other| other.group() == Some(group))
                .count();
            rows.push(Row::Header { group, count });
        }
        previous = group;
        rows.push(Row::Item(index));
    }
    rows
}

#[cfg(test)]
mod tests {
    use margin_kernel::UnixMillis;

    use super::*;

    fn path(raw: &str) -> RelativePath {
        RelativePath::new(raw).expect("valid test path")
    }

    fn entry(raw: &str) -> FsEntry {
        let path = path(raw);
        FsEntry {
            name: path.file_name().to_owned(),
            is_dir: false,
            path,
            modified: UnixMillis::default(),
        }
    }

    fn hit(raw: &str) -> SearchHit {
        SearchHit {
            path: path(raw),
            name: raw.to_owned(),
            snippet: String::new(),
        }
    }

    fn tag(name: &str, files: &[&str]) -> TagInfo {
        TagInfo {
            tag: name.to_owned(),
            count: files.len(),
            files: files.iter().map(|raw| path(raw)).collect(),
        }
    }

    #[test]
    fn names_come_before_contents_and_duplicates_are_dropped() {
        let query = Query::Search("plan".to_owned());
        let names = [entry("plan.md"), entry("work/planning.md")];
        let hits = [hit("work/planning.md"), hit("ideas.md")];
        let items = build_items(&query, None, &[], &names, &hits);
        let paths: Vec<&str> = items
            .iter()
            .map(|item| match item {
                Item::Name { path, .. } | Item::TagFile(path) => path.as_str(),
                Item::Content(hit) => hit.path.as_str(),
                Item::Tag { tag, .. } => tag.as_str(),
            })
            .collect();
        assert_eq!(paths, ["plan.md", "work/planning.md", "ideas.md"]);
        assert!(matches!(items[2], Item::Content(_)));
    }

    #[test]
    fn groups_get_one_header_with_their_count() {
        let query = Query::Search("plan".to_owned());
        let names = [entry("plan.md"), entry("b.md")];
        let hits = [hit("c.md")];
        let items = build_items(&query, None, &[], &names, &hits);
        assert_eq!(
            rows(&items),
            [
                Row::Header {
                    group: Group::Names,
                    count: 2
                },
                Row::Item(0),
                Row::Item(1),
                Row::Header {
                    group: Group::Contents,
                    count: 1
                },
                Row::Item(2),
            ]
        );
    }

    #[test]
    fn tag_mode_filters_tags_by_substring() {
        let query = Query::Tags {
            filter: "pro".to_owned(),
        };
        let tags = [tag("project", &["a.md"]), tag("home", &["b.md"])];
        let items = build_items(&query, None, &tags, &[], &[]);
        assert_eq!(
            items,
            [Item::Tag {
                tag: "project".to_owned(),
                count: 1
            }]
        );
        assert_eq!(
            rows(&items)[0],
            Row::Header {
                group: Group::Tags,
                count: 1
            }
        );
    }

    #[test]
    fn a_selected_tag_lists_its_notes_without_a_header() {
        let query = Query::Tags {
            filter: "project".to_owned(),
        };
        let tags = [tag("project", &["a.md", "b.md"])];
        let items = build_items(&query, Some("Project"), &tags, &[], &[]);
        assert_eq!(
            items,
            [Item::TagFile(path("a.md")), Item::TagFile(path("b.md"))]
        );
        assert_eq!(rows(&items), [Row::Item(0), Row::Item(1)]);
    }

    #[test]
    fn ask_mode_has_no_items() {
        let query = Query::Ask("why".to_owned());
        let names = [entry("why.md")];
        assert!(build_items(&query, None, &[], &names, &[]).is_empty());
    }
}
