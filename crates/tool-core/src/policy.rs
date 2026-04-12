use std::path::{Path, PathBuf};

use anyhow::{Result, bail};

pub(crate) fn canonicalize_user_path(path: &Path, base: Option<&Path>) -> Result<PathBuf> {
    let resolved = if path.is_absolute() {
        path.to_path_buf()
    } else {
        match base {
            Some(base) => base.join(path),
            None => std::env::current_dir()?.join(path),
        }
    };

    if resolved.exists() {
        Ok(resolved.canonicalize()?)
    } else {
        Ok(resolved)
    }
}

pub(crate) fn ensure_allowed_path(
    path: &Path,
    allowed_roots: &[PathBuf],
    tool_name: &str,
) -> Result<()> {
    if allowed_roots.is_empty() {
        return Ok(());
    }

    let candidate = if path.exists() {
        path.canonicalize()?
    } else {
        path.to_path_buf()
    };
    let allowed = allowed_roots.iter().filter_map(|root| {
        if root.exists() {
            root.canonicalize().ok()
        } else {
            Some(root.clone())
        }
    });

    if allowed.into_iter().any(|root| candidate.starts_with(&root)) {
        return Ok(());
    }

    bail!(
        "{} path is outside the allowed capability roots: {}",
        tool_name,
        candidate.display()
    )
}
