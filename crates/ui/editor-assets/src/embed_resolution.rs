use std::path::PathBuf;

use margin_kernel::RelativePath;

use crate::vault_index::VaultIndex;
use crate::vault_paths::{
    decode_destination, extension_of, find_variant, is_image_path, is_remote, is_web_image, joined,
    localfile_path, normalized, note_dir,
};

pub const MAX_EMBED_DEPTH: usize = 2;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct EmbedSize {
    pub width: u32,
    pub height: Option<u32>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum ImageLocation {
    Local(PathBuf),
    Remote(String),
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ResolvedImage {
    pub location: ImageLocation,
    pub size: Option<EmbedSize>,
    pub alt: String,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Embed {
    Image(ResolvedImage),
    Note {
        name: String,
        heading: Option<String>,
        path: Option<RelativePath>,
    },
    File {
        name: String,
        path: Option<RelativePath>,
    },
}

#[must_use]
pub fn embed_size(spec: &str) -> Option<EmbedSize> {
    let spec = spec.trim();
    let (width, height) = match spec.split_once('x') {
        Some((width, height)) => (width, Some(height)),
        None => (spec, None),
    };
    let number = |text: &str| {
        if text.is_empty() || !text.bytes().all(|byte| byte.is_ascii_digit()) {
            return None;
        }
        text.parse::<u32>().ok()
    };
    Some(EmbedSize {
        width: number(width)?,
        height: match height {
            Some(height) => Some(number(height)?),
            None => None,
        },
    })
}

#[must_use]
pub fn resolve_image(
    index: &VaultIndex,
    note: &RelativePath,
    destination: &str,
    label: &str,
) -> Option<ResolvedImage> {
    let clean = destination.replace("\\|", "|");
    let location = if is_web_image(&clean) {
        ImageLocation::Remote(clean.trim().to_owned())
    } else {
        ImageLocation::Local(index.absolute(&asset_path(index, note, &clean)?))
    };
    let text = label.replace("\\|", "|");
    let sized = text
        .rfind('|')
        .and_then(|cut| embed_size(&text[cut + 1..]).map(|size| (size, text[..cut].to_owned())));
    let (size, alt) = match sized {
        Some((size, alt)) => (Some(size), alt),
        None => (None, text),
    };
    Some(ResolvedImage {
        location,
        size,
        alt,
    })
}

#[must_use]
pub fn resolve_embed(index: &VaultIndex, note: &RelativePath, inner: &str) -> Option<Embed> {
    let clean = inner.replace("\\|", "|");
    let (name_part, spec) = clean.split_once('|').unwrap_or((clean.as_str(), ""));
    let name = name_part.trim();
    if is_image_path(&extension_of(name)) {
        let location = if is_web_image(name) {
            ImageLocation::Remote(name.to_owned())
        } else {
            ImageLocation::Local(index.absolute(&embedded_asset(index, name)?))
        };
        let size = embed_size(spec);
        let alt = match (size, spec.trim()) {
            (None, written) if !written.is_empty() => written.to_owned(),
            _ => name.to_owned(),
        };
        return Some(Embed::Image(ResolvedImage {
            location,
            size,
            alt,
        }));
    }
    let (written, heading) = match name_part.split_once('#') {
        Some((written, heading)) => (
            written.trim(),
            Some(heading.trim().to_owned()).filter(|heading| !heading.is_empty()),
        ),
        None => (name, None),
    };
    if written.is_empty() {
        return None;
    }
    let extension = extension_of(written);
    if extension.is_empty() || extension == "md" {
        let variants = if extension == "md" {
            vec![written.to_owned()]
        } else {
            vec![format!("{written}.md"), written.to_owned()]
        };
        let title = if extension == "md" {
            &written[..written.len() - 3]
        } else {
            written
        };
        return Some(Embed::Note {
            name: title.to_owned(),
            heading,
            path: find_variant(index, note, &variants),
        });
    }
    Some(Embed::File {
        name: written.to_owned(),
        path: find_variant(index, note, &[written.to_owned()]),
    })
}

fn asset_path(index: &VaultIndex, note: &RelativePath, source: &str) -> Option<RelativePath> {
    let raw = source.trim();
    if raw.is_empty() || is_remote(raw) {
        return None;
    }
    if let Some(absolute) = localfile_path(raw) {
        return index.relative(&absolute);
    }
    let path = decode_destination(raw);
    if path.starts_with('/') {
        return normalized(&path);
    }
    picked(
        index,
        &[
            joined(&note_dir(note), &path),
            path.clone(),
            joined(index.attachment_folder(), &path),
        ],
    )
}

fn embedded_asset(index: &VaultIndex, name: &str) -> Option<RelativePath> {
    let target = name.split('"').next().unwrap_or(name).trim();
    if target.is_empty() || target.starts_with('#') {
        return None;
    }
    let candidates = [joined(index.attachment_folder(), target), target.to_owned()];
    let existing = candidates
        .iter()
        .filter_map(|candidate| normalized(candidate))
        .find(|path| index.exists(path));
    existing
        .or_else(|| {
            if target.contains('/') {
                None
            } else {
                index.find_by_name(target)
            }
        })
        .or_else(|| picked(index, &candidates))
}

fn picked(index: &VaultIndex, candidates: &[String]) -> Option<RelativePath> {
    let mut unique: Vec<RelativePath> = Vec::new();
    for candidate in candidates {
        let Some(path) = normalized(candidate) else {
            continue;
        };
        if index.exists(&path) {
            return Some(path);
        }
        if !unique.contains(&path) {
            unique.push(path);
        }
    }
    if unique.len() != 1 && !index.is_ready() {
        return None;
    }
    unique.into_iter().next()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn path(raw: &str) -> RelativePath {
        RelativePath::new(raw).expect("test paths are valid")
    }

    fn index() -> VaultIndex {
        VaultIndex::new(
            PathBuf::from("/vault"),
            path("attachments"),
            [
                path("media/shot.png"),
                path("attachments/logo.png"),
                path("notes/pic.jpg"),
                path("Ideas.md"),
                path("files/report.pdf"),
            ],
        )
    }

    fn local(raw: &str) -> ImageLocation {
        ImageLocation::Local(PathBuf::from(raw))
    }

    #[test]
    fn sizes_are_width_or_width_by_height() {
        assert_eq!(
            embed_size(" 300 "),
            Some(EmbedSize {
                width: 300,
                height: None
            })
        );
        assert_eq!(
            embed_size("300x200"),
            Some(EmbedSize {
                width: 300,
                height: Some(200)
            })
        );
        assert_eq!(embed_size("300x"), None);
        assert_eq!(embed_size("big"), None);
        assert_eq!(embed_size("+3"), None);
    }

    #[test]
    fn image_embeds_resolve_through_the_attachment_folder_and_names() {
        let index = index();
        let note = path("notes/Today.md");
        assert_eq!(
            resolve_embed(&index, &note, "logo.png|200"),
            Some(Embed::Image(ResolvedImage {
                location: local("/vault/attachments/logo.png"),
                size: Some(EmbedSize {
                    width: 200,
                    height: None
                }),
                alt: String::from("logo.png"),
            }))
        );
        assert_eq!(
            resolve_embed(&index, &note, "shot.png\\|Caption"),
            Some(Embed::Image(ResolvedImage {
                location: local("/vault/media/shot.png"),
                size: None,
                alt: String::from("Caption"),
            }))
        );
        assert_eq!(
            resolve_embed(&index, &note, "https://x.test/a.png"),
            Some(Embed::Image(ResolvedImage {
                location: ImageLocation::Remote(String::from("https://x.test/a.png")),
                size: None,
                alt: String::from("https://x.test/a.png"),
            }))
        );
    }

    #[test]
    fn note_and_file_embeds_keep_their_heading_and_name() {
        let index = index();
        let note = path("notes/Today.md");
        assert_eq!(
            resolve_embed(&index, &note, "Ideas#Later|alias"),
            Some(Embed::Note {
                name: String::from("Ideas"),
                heading: Some(String::from("Later")),
                path: Some(path("Ideas.md")),
            })
        );
        assert_eq!(
            resolve_embed(&index, &note, "Missing.md"),
            Some(Embed::Note {
                name: String::from("Missing"),
                heading: None,
                path: None,
            })
        );
        assert_eq!(
            resolve_embed(&index, &note, "report.pdf"),
            Some(Embed::File {
                name: String::from("report.pdf"),
                path: Some(path("files/report.pdf")),
            })
        );
        assert_eq!(resolve_embed(&index, &note, "#only"), None);
    }

    #[test]
    fn markdown_images_resolve_from_the_note_folder_with_label_sizes() {
        let index = index();
        let note = path("notes/Today.md");
        assert_eq!(
            resolve_image(&index, &note, "pic.jpg", "A pic|120x80"),
            Some(ResolvedImage {
                location: local("/vault/notes/pic.jpg"),
                size: Some(EmbedSize {
                    width: 120,
                    height: Some(80)
                }),
                alt: String::from("A pic"),
            })
        );
        assert_eq!(
            resolve_image(&index, &note, "/media/shot.png", "a|b"),
            Some(ResolvedImage {
                location: local("/vault/media/shot.png"),
                size: None,
                alt: String::from("a|b"),
            })
        );
        assert_eq!(resolve_image(&index, &note, "blob:xyz", ""), None);
    }
}
