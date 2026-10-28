use std::path::{Path, PathBuf};
use std::sync::Arc;

use gpui_kit::{App, Task};
use margin_attachments::{AttachmentsService, Error};
use margin_kernel::RelativePath;
use margin_workbench::{AppServices, CurrentVault, run_blocking};

use crate::vault_index::VaultIndex;
use crate::vault_paths::inside_vault;

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum AttachmentSource {
    Bytes {
        bytes: Vec<u8>,
        name: Option<String>,
        extension: String,
    },
    Path(PathBuf),
}

#[derive(Debug)]
pub struct AttachmentFailure {
    pub name: String,
    pub error: Error,
}

#[derive(Debug)]
enum Stored {
    Text(String),
    Attachment(RelativePath),
}

pub fn store_attachments(
    sources: Vec<AttachmentSource>,
    cx: &App,
) -> Task<Vec<Result<String, AttachmentFailure>>> {
    let Some(vault) = CurrentVault::get(cx) else {
        return Task::ready(Vec::new());
    };
    let folder = vault.app_settings.read().attachment_folder();
    let now = cx.try_global::<AppServices>().map(|app| app.clock.now());
    let stamp = now.map_or_else(|| String::from("0"), |now| now.to_string());
    let service = Arc::clone(&vault.attachments);
    let root = vault.root.clone();
    let storing = run_blocking(cx, move || {
        sources
            .into_iter()
            .map(|source| stored(&service, &root, &folder, &stamp, source))
            .collect::<Vec<_>>()
    });
    cx.spawn(async move |cx| {
        let stored = storing.await;
        cx.update(|cx| {
            let texts: Vec<Result<String, AttachmentFailure>> = stored
                .into_iter()
                .map(|result| {
                    result.map(|stored| match stored {
                        Stored::Text(text) => text,
                        Stored::Attachment(path) => embed_for(VaultIndex::get(cx), &path),
                    })
                })
                .collect();
            VaultIndex::refresh(cx);
            texts
        })
    })
}

fn stored(
    service: &AttachmentsService,
    root: &Path,
    folder: &RelativePath,
    stamp: &str,
    source: AttachmentSource,
) -> Result<Stored, AttachmentFailure> {
    match source {
        AttachmentSource::Bytes {
            bytes,
            name,
            extension,
        } => {
            let name = name.unwrap_or_else(|| pasted_name(stamp, &extension));
            service
                .store(folder, &name, &bytes)
                .map(Stored::Attachment)
                .map_err(|error| AttachmentFailure { name, error })
        }
        AttachmentSource::Path(path) => {
            let name = path
                .file_name()
                .map_or_else(String::new, |name| name.to_string_lossy().into_owned());
            if let Some(title) = note_title(&name) {
                return Ok(Stored::Text(format!("[[{title}]]")));
            }
            if let Some(inside) = inside_vault(root, &path) {
                return Ok(Stored::Attachment(inside));
            }
            service
                .import_file(&path, folder)
                .map(Stored::Attachment)
                .map_err(|error| AttachmentFailure { name, error })
        }
    }
}

fn pasted_name(stamp: &str, extension: &str) -> String {
    let extension = extension.split('+').next().unwrap_or_default();
    let extension = if extension.is_empty() {
        "png"
    } else {
        extension
    };
    format!("pasted-{stamp}.{extension}")
}

fn note_title(name: &str) -> Option<&str> {
    let (stem, extension) = name.rsplit_once('.')?;
    let lower = extension.to_lowercase();
    (lower == "md" || lower == "canvas").then_some(stem)
}

fn embed_for(index: Option<&VaultIndex>, path: &RelativePath) -> String {
    let name = path.file_name();
    match index.and_then(|index| index.find_by_name(name)) {
        Some(known) if known != *path => format!("![[{path}]]"),
        _ => format!("![[{name}]]"),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn path(raw: &str) -> RelativePath {
        RelativePath::new(raw).expect("test paths are valid")
    }

    #[test]
    fn embeds_spell_out_the_path_only_when_the_name_is_ambiguous() {
        let index = VaultIndex::new(
            PathBuf::from("/vault"),
            path("attachments"),
            [path("attachments/shot.png"), path("old/logo.png")],
        );
        let index = Some(&index);
        assert_eq!(
            embed_for(index, &path("attachments/shot.png")),
            "![[shot.png]]"
        );
        assert_eq!(
            embed_for(index, &path("attachments/new.png")),
            "![[new.png]]"
        );
        assert_eq!(
            embed_for(index, &path("attachments/logo.png")),
            "![[attachments/logo.png]]"
        );
    }

    #[test]
    fn dropped_notes_link_and_vault_files_are_not_copied() {
        assert_eq!(note_title("Plan.md"), Some("Plan"));
        assert_eq!(note_title("Board.CANVAS"), Some("Board"));
        assert_eq!(note_title("shot.png"), None);
        assert_eq!(
            inside_vault(Path::new("/vault"), Path::new("/vault/media/a.png")),
            Some(path("media/a.png"))
        );
        assert_eq!(
            inside_vault(Path::new("/vault"), Path::new("/tmp/a.png")),
            None
        );
    }

    #[test]
    fn pasted_images_are_named_by_time_and_type() {
        assert_eq!(pasted_name("42", "svg+xml"), "pasted-42.svg");
        assert_eq!(pasted_name("42", ""), "pasted-42.png");
    }
}
