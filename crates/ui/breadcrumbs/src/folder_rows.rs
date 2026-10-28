use margin_files::{FsEntry, TreeRow};
use margin_kernel::RelativePath;

const MAX_MATCHES: usize = 200;

pub(crate) fn matching_rows(
    entries: Vec<FsEntry>,
    filter: &str,
    hidden: &RelativePath,
) -> Vec<TreeRow> {
    let needle = filter.to_lowercase();
    let mut rows: Vec<TreeRow> = entries
        .into_iter()
        .filter(|entry| {
            !entry.path.is_hidden()
                && !entry.path.starts_with(hidden)
                && entry.name.to_lowercase().contains(&needle)
        })
        .map(|entry| TreeRow {
            name: entry.name,
            path: entry.path,
            is_dir: entry.is_dir,
            modified: entry.modified,
            depth: 0,
        })
        .collect();
    rows.sort_by_cached_key(|row| (!row.is_dir, row.name.to_lowercase()));
    rows.truncate(MAX_MATCHES);
    rows
}

pub(crate) fn parent_index(rows: &[TreeRow], index: usize) -> Option<usize> {
    let depth = rows.get(index)?.depth;
    rows.get(..index)?.iter().rposition(|row| row.depth < depth)
}

#[cfg(test)]
mod tests {
    use margin_kernel::UnixMillis;

    use super::*;

    fn entry(raw: &str, is_dir: bool) -> FsEntry {
        let path = RelativePath::new(raw).expect("valid path");
        FsEntry {
            name: path.file_name().to_owned(),
            is_dir,
            path,
            modified: UnixMillis::default(),
        }
    }

    fn names(rows: &[TreeRow]) -> Vec<&str> {
        rows.iter().map(|row| row.path.as_str()).collect()
    }

    fn hidden() -> RelativePath {
        RelativePath::new("attachments").expect("valid path")
    }

    #[test]
    fn filtering_matches_names_case_insensitively_folders_first() {
        let entries = vec![
            entry("notes/plan.md", false),
            entry("notes/Planning", true),
            entry("notes/Planning/Alpha.md", false),
            entry("notes/other.md", false),
        ];

        let rows = matching_rows(entries, "PLAN", &hidden());

        assert_eq!(names(&rows), ["notes/Planning", "notes/plan.md"]);
        assert!(rows.iter().all(|row| row.depth == 0));
    }

    #[test]
    fn filtering_never_shows_hidden_paths_or_the_attachment_folder() {
        let entries = vec![
            entry("attachments", true),
            entry("attachments/a.png", false),
            entry(".margin/a.md", false),
            entry("a.md", false),
        ];

        let rows = matching_rows(entries, "a", &hidden());

        assert_eq!(names(&rows), ["a.md"]);
    }

    #[test]
    fn filtering_caps_the_number_of_matches() {
        let entries = (0..300)
            .map(|index| entry(&format!("note-{index}.md"), false))
            .collect();

        assert_eq!(matching_rows(entries, "note", &hidden()).len(), MAX_MATCHES);
    }

    #[test]
    fn parent_index_finds_the_nearest_shallower_row() {
        let row = |raw: &str, depth: usize| TreeRow {
            depth,
            ..matching_rows(vec![entry(raw, true)], "", &hidden()).remove(0)
        };
        let rows = [row("a", 0), row("a/b", 1), row("a/b/c", 2), row("a/d", 1)];

        assert_eq!(parent_index(&rows, 3), Some(0));
        assert_eq!(parent_index(&rows, 2), Some(1));
        assert_eq!(parent_index(&rows, 0), None);
        assert_eq!(parent_index(&rows, 9), None);
    }
}
