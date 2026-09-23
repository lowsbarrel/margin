use std::path::{Component, Path, PathBuf};
use std::sync::Mutex;

pub struct VaultPathState(pub Mutex<String>);

#[inline]
pub(crate) fn path_to_string(p: PathBuf) -> String {
    let s = p
        .into_os_string()
        .into_string()
        .unwrap_or_else(|s| s.to_string_lossy().into_owned());
    #[cfg(target_os = "windows")]
    {
        s.replace('\\', "/")
    }
    #[cfg(not(target_os = "windows"))]
    {
        s
    }
}

pub(crate) fn normalise_slashes(p: &str) -> String {
    p.replace('\\', "/")
}

pub(crate) fn valid_rel_path(rel: &str) -> bool {
    !rel.is_empty()
        && !rel.contains('\\')
        && Path::new(rel).components().all(|c| match c {
            Component::Normal(name) => name
                .to_str()
                .map(|s| !s.is_empty() && !s.starts_with('.'))
                .unwrap_or(false),
            _ => false,
        })
}

pub(crate) fn rel_under_vault(vault: &str, candidate: &str) -> Option<String> {
    let vault = vault.trim_end_matches('/');
    let candidate = candidate.trim_end_matches('/');
    let vault_path = Path::new(vault);

    let Ok(rel) = Path::new(candidate).strip_prefix(vault_path) else {
        #[cfg(target_os = "windows")]
        return rel_under_vault_case_folded(vault_path, candidate);
        #[cfg(not(target_os = "windows"))]
        return None;
    };
    if rel.as_os_str().is_empty() {
        return None;
    }
    Some(rel.to_string_lossy().replace('\\', "/"))
}

#[cfg(target_os = "windows")]
fn rel_under_vault_case_folded(vault_path: &Path, candidate: &str) -> Option<String> {
    let v: Vec<&std::ffi::OsStr> = vault_path.components().map(|c| c.as_os_str()).collect();
    let c: Vec<&std::ffi::OsStr> = Path::new(candidate)
        .components()
        .map(|c| c.as_os_str())
        .collect();
    if c.len() <= v.len()
        || !v
            .iter()
            .zip(&c)
            .all(|(a, b)| a.to_string_lossy().to_lowercase() == b.to_string_lossy().to_lowercase())
    {
        return None;
    }
    let rel: PathBuf = c[v.len()..].iter().copied().collect();
    Some(rel.to_string_lossy().replace('\\', "/"))
}

pub(crate) fn ensure_in_vault(path: &str, vault: &VaultPathState) -> Result<PathBuf, String> {
    let vault_root = vault
        .0
        .lock()
        .map_err(|e| e.to_string())?
        .trim()
        .to_string();
    ensure_within(&vault_root, path)
}

pub(crate) fn ensure_within(root: &str, path: &str) -> Result<PathBuf, String> {
    let mut had_parent_dir = false;
    let target = Path::new(path);
    for component in target.components() {
        if matches!(component, Component::ParentDir) {
            had_parent_dir = true;
            break;
        }
    }
    if had_parent_dir {
        return Err("Path escapes the vault".into());
    }
    // Startup writes .margin/ before set_vault_directory, so there is no boundary to enforce yet.
    if root.trim().is_empty() {
        return Ok(target.to_path_buf());
    }

    let canonical_vault = Path::new(root.trim())
        .canonicalize()
        .map_err(|e| format!("Failed to resolve vault root: {e}"))?;

    let mut ancestor = target;
    let mut tail: Vec<&std::ffi::OsStr> = Vec::new();
    let mut resolved = loop {
        match ancestor.canonicalize() {
            Ok(c) => break c,
            Err(_) => match ancestor.parent() {
                Some(parent) => {
                    if let Some(name) = ancestor.file_name() {
                        tail.push(name);
                    }
                    ancestor = parent;
                }
                None => return Err("Failed to resolve path".into()),
            },
        }
    };
    for name in tail.into_iter().rev() {
        resolved.push(name);
    }

    if !resolved.starts_with(&canonical_vault) {
        return Err("Path escapes the vault".into());
    }
    Ok(resolved)
}

// macOS realpath folds a case-only rename back onto the source name, so the last component is kept as spelled.
pub(crate) fn ensure_in_vault_keep_name(
    path: &str,
    vault: &VaultPathState,
) -> Result<PathBuf, String> {
    let target = Path::new(path);
    let (Some(parent), Some(name)) = (target.parent(), target.file_name()) else {
        return Err("Invalid path".into());
    };
    let mut resolved = ensure_in_vault(&parent.to_string_lossy(), vault)?;
    resolved.push(name);
    Ok(resolved)
}

pub(crate) fn vault_root(vault_path_state: &tauri::State<'_, VaultPathState>) -> String {
    vault_path_state
        .0
        .lock()
        .map(|v| v.clone())
        .unwrap_or_default()
}
