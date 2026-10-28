use margin_kernel::RelativePath;

pub(crate) fn remap(path: &RelativePath, from: &RelativePath, to: &RelativePath) -> RelativePath {
    match path.strip_prefix(from) {
        Some(rest) if !from.is_root() => to
            .join(rest.as_str())
            .expect("a relative remainder joins onto the new prefix"),
        _ => path.clone(),
    }
}

pub(crate) fn parent(path: &RelativePath) -> RelativePath {
    path.parent().unwrap_or_default()
}

pub(crate) fn ancestors(path: &RelativePath) -> Vec<RelativePath> {
    let mut found = Vec::new();
    let mut current = path.parent();
    while let Some(folder) = current.filter(|folder| !folder.is_root()) {
        current = folder.parent();
        found.push(folder);
    }
    found.reverse();
    found
}

pub(crate) fn child(folder: &RelativePath, name: &str) -> RelativePath {
    folder
        .join(name)
        .expect("a validated name joins its folder")
}

#[cfg(test)]
mod tests {
    use super::*;

    fn path(raw: &str) -> RelativePath {
        RelativePath::new(raw).expect("valid path")
    }

    #[test]
    fn remaps_the_entry_and_its_descendants_only() {
        let from = path("a/b");
        let to = path("c");
        assert_eq!(remap(&path("a/b"), &from, &to), path("c"));
        assert_eq!(remap(&path("a/b/x.md"), &from, &to), path("c/x.md"));
        assert_eq!(remap(&path("a/bc.md"), &from, &to), path("a/bc.md"));
    }

    #[test]
    fn lists_ancestors_from_the_top() {
        assert_eq!(ancestors(&path("a/b/c.md")), [path("a"), path("a/b")]);
        assert!(ancestors(&path("c.md")).is_empty());
    }
}
