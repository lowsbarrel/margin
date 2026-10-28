use std::sync::Arc;

use gpui_kit::{App, Global};
use margin_files::FilesService;
use margin_kernel::RelativePath;
use margin_workbench::{CurrentVault, run_blocking};

use crate::generations::Generations;
use crate::vault_index::VaultIndex;

const CACHED_FILES: usize = 64;

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum NoteTextState {
    Pending,
    Ready(Arc<str>),
    Failed,
}

#[derive(Debug)]
struct Loaded<V> {
    value: Option<V>,
    generation: u64,
    loading: bool,
}

#[derive(Debug)]
struct VaultFiles<V> {
    entries: Generations<RelativePath, Loaded<V>>,
}

impl<V> Default for VaultFiles<V> {
    fn default() -> Self {
        Self {
            entries: Generations::with_capacity(CACHED_FILES),
        }
    }
}

impl<V: 'static> Global for VaultFiles<V> {}

#[must_use]
pub fn note_text(path: &RelativePath, cx: &mut App) -> NoteTextState {
    let loaded = load(path, cx, |files, path| {
        files
            .read_file(path)
            .ok()
            .map(|bytes| Arc::from(String::from_utf8_lossy(&bytes).as_ref()))
    });
    match loaded {
        None => NoteTextState::Pending,
        Some(Some(text)) => NoteTextState::Ready(text),
        Some(None) => NoteTextState::Failed,
    }
}

#[must_use]
pub fn file_size(path: &RelativePath, cx: &mut App) -> Option<u64> {
    load(path, cx, |files, path| {
        files.metadata(path).ok().map(|metadata| metadata.size)
    })
    .flatten()
}

fn load<V: Clone + Send + 'static>(
    path: &RelativePath,
    cx: &mut App,
    read: fn(&FilesService, &RelativePath) -> V,
) -> Option<V> {
    let vault = CurrentVault::get(cx)?;
    let generation = VaultIndex::get(cx).map_or(0, VaultIndex::generation);
    let cache = cx.default_global::<VaultFiles<V>>();
    let value = match cache.entries.get_mut(path) {
        Some(entry) if entry.generation == generation || entry.loading => {
            return entry.value.clone();
        }
        Some(entry) => {
            entry.loading = true;
            entry.value.clone()
        }
        None => {
            cache.entries.insert(
                path.clone(),
                Loaded {
                    value: None,
                    generation,
                    loading: true,
                },
            );
            None
        }
    };
    let files = Arc::clone(&vault.files);
    let key = path.clone();
    let task_path = path.clone();
    let reading = run_blocking(cx, move || read(&files, &task_path));
    cx.spawn(async move |cx| {
        let loaded = reading.await;
        cx.update_global::<VaultFiles<V>, _>(|cache, _| {
            cache.entries.insert(
                key,
                Loaded {
                    value: Some(loaded),
                    generation,
                    loading: false,
                },
            );
        });
    })
    .detach();
    value
}

#[must_use]
pub fn heading_section<'text>(text: &'text str, heading: &str) -> &'text str {
    let wanted = heading.trim().to_lowercase();
    let mut start: Option<(usize, usize)> = None;
    let mut offset = 0;
    for line in text.split_inclusive('\n') {
        let line_start = offset;
        offset += line.len();
        let Some((level, title)) = atx_heading(line.trim_end_matches(['\n', '\r'])) else {
            continue;
        };
        match start {
            None if title.to_lowercase() == wanted => start = Some((line_start, level)),
            Some((from, open_level)) if level <= open_level => {
                let section = &text[from..line_start];
                return section.strip_suffix('\n').unwrap_or(section);
            }
            _ => {}
        }
    }
    start.map_or(text, |(from, _)| &text[from..])
}

fn atx_heading(line: &str) -> Option<(usize, &str)> {
    let level = line.bytes().take_while(|byte| *byte == b'#').count();
    if !(1..=6).contains(&level) {
        return None;
    }
    let rest = &line[level..];
    if !rest.starts_with([' ', '\t']) {
        return None;
    }
    let title = rest
        .trim_end_matches([' ', '\t'])
        .trim_end_matches('#')
        .trim_matches([' ', '\t']);
    Some((level, title))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_heading_section_runs_until_the_next_heading_of_its_level() {
        let text = "intro\n## Plan ##\nstep\n### Detail\nmore\n## Next\nafter\n";
        assert_eq!(
            heading_section(text, " plan "),
            "## Plan ##\nstep\n### Detail\nmore"
        );
        assert_eq!(heading_section(text, "Next"), "## Next\nafter\n");
        assert_eq!(heading_section(text, "Missing"), text);
        assert_eq!(heading_section("#Tag\ntext", "Tag"), "#Tag\ntext");
    }
}
