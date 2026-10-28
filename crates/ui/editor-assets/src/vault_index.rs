use std::collections::{HashMap, HashSet};
use std::path::{Path, PathBuf};
use std::sync::Arc;

use gpui_kit::{App, Global};
use margin_kernel::RelativePath;
use margin_workbench::{CurrentVault, run_blocking};

use crate::vault_paths::inside_vault;

#[derive(Debug, Default)]
pub struct VaultIndex {
    root: PathBuf,
    attachment_folder: RelativePath,
    files: HashSet<RelativePath>,
    by_name: HashMap<String, RelativePath>,
    ready: bool,
    generation: u64,
    walking: bool,
    rerun: bool,
}

impl Global for VaultIndex {}

impl VaultIndex {
    #[must_use]
    pub fn new(
        root: PathBuf,
        attachment_folder: RelativePath,
        files: impl IntoIterator<Item = RelativePath>,
    ) -> Self {
        let mut index = Self {
            root,
            attachment_folder,
            ..Self::default()
        };
        index.fill(files.into_iter().collect());
        index
    }

    #[must_use]
    pub fn get(cx: &App) -> Option<&Self> {
        cx.try_global::<Self>()
    }

    #[must_use]
    pub fn expect(cx: &App) -> &Self {
        Self::get(cx).expect("VaultIndex::refresh runs when the editor opens")
    }

    pub fn refresh(cx: &mut App) {
        let Some(vault) = CurrentVault::get(cx) else {
            return;
        };
        let attachment_folder = vault.app_settings.read().attachment_folder();
        let index = cx.default_global::<Self>();
        if index.root != vault.root {
            *index = Self {
                root: vault.root.clone(),
                ..Self::default()
            };
        }
        index.attachment_folder = attachment_folder;
        if index.walking {
            index.rerun = true;
            return;
        }
        index.walking = true;
        let root = vault.root.clone();
        let files = Arc::clone(&vault.files);
        let walk = run_blocking(cx, move || {
            files
                .walk_directory(&RelativePath::root(), false)
                .into_iter()
                .filter(|entry| !entry.is_dir)
                .map(|entry| entry.path)
                .collect::<Vec<_>>()
        });
        cx.spawn(async move |cx| {
            let files = walk.await;
            let rerun = cx.update_global::<Self, _>(|index, _| {
                if index.root != root {
                    return false;
                }
                index.fill(files);
                index.walking = false;
                index.generation += 1;
                let rerun = index.rerun;
                index.rerun = false;
                rerun
            });
            if rerun {
                cx.update(Self::refresh);
            }
        })
        .detach();
    }

    fn fill(&mut self, mut files: Vec<RelativePath>) {
        files.sort_by(|left, right| {
            left.components()
                .count()
                .cmp(&right.components().count())
                .then_with(|| left.cmp(right))
        });
        self.by_name.clear();
        for path in &files {
            self.by_name
                .entry(path.file_name().to_lowercase())
                .or_insert_with(|| path.clone());
        }
        self.files = files.into_iter().collect();
        self.ready = true;
    }

    #[must_use]
    pub fn exists(&self, path: &RelativePath) -> bool {
        self.files.contains(path)
    }

    #[must_use]
    pub fn find_by_name(&self, name: &str) -> Option<RelativePath> {
        self.by_name.get(&name.to_lowercase()).cloned()
    }

    #[must_use]
    pub fn is_ready(&self) -> bool {
        self.ready
    }

    #[must_use]
    pub fn generation(&self) -> u64 {
        self.generation
    }

    #[must_use]
    pub fn root(&self) -> &Path {
        &self.root
    }

    #[must_use]
    pub fn attachment_folder(&self) -> &RelativePath {
        &self.attachment_folder
    }

    #[must_use]
    pub fn absolute(&self, path: &RelativePath) -> PathBuf {
        path.components()
            .fold(self.root.clone(), |absolute, part| absolute.join(part))
    }

    #[must_use]
    pub fn relative(&self, absolute: &Path) -> Option<RelativePath> {
        inside_vault(&self.root, absolute)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn path(raw: &str) -> RelativePath {
        RelativePath::new(raw).expect("test paths are valid")
    }

    #[test]
    fn a_name_matches_the_shortest_path_case_insensitively() {
        let index = VaultIndex::new(
            PathBuf::from("/vault"),
            path("attachments"),
            [
                path("deep/inner/Note.md"),
                path("b/note.md"),
                path("a/Note.md"),
            ],
        );
        assert_eq!(index.find_by_name("NOTE.md"), Some(path("a/Note.md")));
        assert!(index.exists(&path("b/note.md")));
        assert!(!index.exists(&path("note.md")));
        assert_eq!(index.find_by_name("missing.md"), None);
    }

    #[test]
    fn absolute_paths_map_to_and_from_the_vault() {
        let index = VaultIndex::new(PathBuf::from("/vault"), path("attachments"), []);
        let absolute = index.absolute(&path("a/b.png"));
        assert_eq!(absolute, PathBuf::from("/vault/a/b.png"));
        assert_eq!(index.relative(&absolute), Some(path("a/b.png")));
        assert_eq!(index.relative(Path::new("/elsewhere/b.png")), None);
    }
}
