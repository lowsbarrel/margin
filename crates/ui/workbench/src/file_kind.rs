use margin_kernel::RelativePath;

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum FileKind {
    Note,
    Canvas,
    Image,
    Pdf,
    Other,
}

impl FileKind {
    #[must_use]
    pub fn of(path: &RelativePath) -> Self {
        let Some(extension) = path.extension().map(str::to_ascii_lowercase) else {
            return Self::Other;
        };
        match extension.as_str() {
            "md" | "markdown" | "txt" => Self::Note,
            "canvas" => Self::Canvas,
            "png" | "jpg" | "jpeg" | "gif" | "webp" | "bmp" | "svg" | "ico" | "avif" => Self::Image,
            "pdf" => Self::Pdf,
            _ => Self::Other,
        }
    }
}

#[must_use]
pub fn file_title(path: &RelativePath) -> &str {
    let name = path.file_name();
    name.strip_suffix(".md")
        .or_else(|| name.strip_suffix(".canvas"))
        .unwrap_or(name)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn kind(path: &str) -> FileKind {
        FileKind::of(&RelativePath::new(path).expect("valid path"))
    }

    #[test]
    fn classifies_by_extension_case_insensitively() {
        assert_eq!(kind("notes/a.MD"), FileKind::Note);
        assert_eq!(kind("board.canvas"), FileKind::Canvas);
        assert_eq!(kind("attachments/x.JPEG"), FileKind::Image);
        assert_eq!(kind("paper.pdf"), FileKind::Pdf);
        assert_eq!(kind("archive.zip"), FileKind::Other);
        assert_eq!(kind("Makefile"), FileKind::Other);
    }

    #[test]
    fn titles_hide_only_note_and_canvas_extensions() {
        let title =
            |path: &str| file_title(&RelativePath::new(path).expect("valid path")).to_owned();
        assert_eq!(title("Projects/Roadmap.md"), "Roadmap");
        assert_eq!(title("Board.canvas"), "Board");
        assert_eq!(title("attachments/photo.png"), "photo.png");
    }
}
