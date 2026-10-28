use margin_kernel::RelativePath;
use margin_workbench::file_title;

#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct Crumb {
    label: String,
    path: RelativePath,
    is_dir: bool,
}

impl Crumb {
    pub(crate) fn label(&self) -> &str {
        &self.label
    }

    pub(crate) fn path(&self) -> &RelativePath {
        &self.path
    }

    pub(crate) fn listed_folder(&self) -> RelativePath {
        if self.is_dir {
            self.path.clone()
        } else {
            self.path.parent().unwrap_or_default()
        }
    }
}

pub(crate) fn crumbs(path: &RelativePath, vault_name: &str) -> Vec<Crumb> {
    if path.is_root() {
        return Vec::new();
    }
    let mut crumbs = vec![Crumb {
        label: vault_name.to_owned(),
        path: RelativePath::root(),
        is_dir: true,
    }];
    let mut folder = RelativePath::root();
    let mut parts = path.components().peekable();
    while let Some(part) = parts.next() {
        if parts.peek().is_none() {
            break;
        }
        folder = folder
            .join(part)
            .expect("a component of a valid path joins");
        crumbs.push(Crumb {
            label: part.to_owned(),
            path: folder.clone(),
            is_dir: true,
        });
    }
    crumbs.push(Crumb {
        label: file_title(path).to_owned(),
        path: path.clone(),
        is_dir: false,
    });
    crumbs
}

#[cfg(test)]
mod tests {
    use super::*;

    fn path(raw: &str) -> RelativePath {
        RelativePath::new(raw).expect("valid path")
    }

    #[test]
    fn a_nested_note_yields_vault_folders_and_title() {
        let crumbs = crumbs(&path("projects/2026/plan.md"), "Notes");

        let labels: Vec<&str> = crumbs.iter().map(|crumb| crumb.label.as_str()).collect();
        assert_eq!(labels, ["Notes", "projects", "2026", "plan"]);
        assert_eq!(crumbs[0].path, RelativePath::root());
        assert_eq!(crumbs[2].path, path("projects/2026"));
        assert!(crumbs[2].is_dir);
        assert!(!crumbs[3].is_dir);
        assert_eq!(crumbs[3].listed_folder(), path("projects/2026"));
        assert_eq!(crumbs[1].listed_folder(), path("projects"));
    }

    #[test]
    fn a_top_level_file_lists_the_vault_root() {
        let crumbs = crumbs(&path("board.canvas"), "Notes");

        assert_eq!(crumbs.len(), 2);
        assert_eq!(crumbs[1].label, "board");
        assert_eq!(crumbs[1].listed_folder(), RelativePath::root());
    }

    #[test]
    fn the_root_has_no_crumbs() {
        assert!(crumbs(&RelativePath::root(), "Notes").is_empty());
    }
}
