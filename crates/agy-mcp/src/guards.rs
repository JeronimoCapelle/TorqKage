//! Composable security guards, path confinement, and UTF-8 truncation primitives.

use std::path::{Component, Path, PathBuf};

/// Inspects a target path against dangerous files, secrets, and repository internals.
///
/// # Errors
/// Returns an error if the path attempts to access sensitive files or `.git/`.
pub(crate) fn check_dangerous_files(path: &Path) -> Result<(), String> {
    let lossy = path.to_string_lossy().to_lowercase();

    // Check for sensitive filenames and patterns
    let dangerous_tokens = [
        ".env",
        "id_rsa",
        "id_ed25519",
        "id_ecdsa",
        ".git/",
        ".git\\",
        "secret",
        "token",
        "credential",
        "password",
        ".bashrc",
        ".zshrc",
        ".profile",
    ];

    for token in dangerous_tokens {
        if lossy.contains(token) {
            return Err(format!(
                "Security policy violation: Access to sensitive file pattern '{token}' is blocked."
            ));
        }
    }

    Ok(())
}

/// Resolves and confines a relative path within the canonical workspace bounds.
///
/// # Errors
/// Returns an error if the path escapes workspace bounds, traverses `..`, or violates directory allowlists.
pub(crate) fn resolve_safe_path(
    workspace_root: &Path,
    rel_path_str: &str,
    allowed_directories: &[String],
    enforce_bounds: bool,
) -> Result<PathBuf, String> {
    let clean_str = rel_path_str.trim();
    if clean_str.is_empty() {
        return Err("Path argument cannot be empty".to_owned());
    }

    let input_path = Path::new(clean_str);

    // Reject any raw parent component traversal attempts
    for component in input_path.components() {
        if matches!(component, Component::ParentDir) {
            return Err("Security policy violation: Path traversal via '..' is forbidden".to_owned());
        }
    }

    let candidate = if input_path.is_absolute() {
        input_path.to_path_buf()
    } else {
        workspace_root.join(input_path)
    };

    if enforce_bounds {
        // If file exists, check canonical root prefix
        if candidate.exists() {
            let canonical = candidate
                .canonicalize()
                .map_err(|err| format!("Failed to canonicalize path '{}': {err}", candidate.display()))?;

            let canonical_root = workspace_root
                .canonicalize()
                .unwrap_or_else(|_| workspace_root.to_path_buf());

            if !canonical.starts_with(&canonical_root) {
                return Err(format!(
                    "Security policy violation: Path '{}' escapes canonical workspace root '{}'",
                    canonical.display(),
                    canonical_root.display()
                ));
            }
        } else {
            // For not-yet-existing files, verify parent
            if let Some(parent) = candidate.parent()
                && parent.exists()
            {
                let canonical_parent = parent.canonicalize().map_err(|err| {
                    format!("Failed to canonicalize parent '{}': {err}", parent.display())
                })?;
                let canonical_root = workspace_root
                    .canonicalize()
                    .unwrap_or_else(|_| workspace_root.to_path_buf());

                if !canonical_parent.starts_with(&canonical_root) {
                    return Err(
                        "Security policy violation: Path parent escapes workspace bounds".to_owned(),
                    );
                }
            }
        }
    }

    // Check allowed directories whitelist
    if !allowed_directories.is_empty() {
        let relative = candidate
            .strip_prefix(workspace_root)
            .unwrap_or(input_path);

        let rel_lossy = relative.to_string_lossy();
        let allowed = allowed_directories.iter().any(|dir| {
            rel_lossy == *dir
                || rel_lossy.starts_with(&format!("{dir}/"))
                || rel_lossy.starts_with(&format!("{dir}\\"))
                || rel_lossy.contains(&format!("/{dir}/"))
                || rel_lossy.contains(&format!("\\{dir}\\"))
        });

        if !allowed {
            return Err(format!(
                "Security policy violation: Path '{rel_lossy}' is outside allowed directories: {allowed_directories:?}"
            ));
        }
    }

    Ok(candidate)
}

/// Truncates string output by lines and bytes safely on UTF-8 boundaries.
pub(crate) fn truncate_output(
    text: &str,
    max_lines: Option<usize>,
    max_bytes: Option<usize>,
    emit_notice: bool,
) -> String {
    let total_lines = text.lines().count();
    let total_bytes = text.len();

    let mut truncated = false;

    // 1. Line-based truncation
    let mut truncated_text = if let Some(limit_lines) = max_lines
        && total_lines > limit_lines
    {
        truncated = true;
        let lines: Vec<&str> = text.lines().take(limit_lines).collect();
        lines.join("\n")
    } else {
        text.to_owned()
    };

    // 2. Byte-based truncation with safe UTF-8 character boundary slicing
    if let Some(limit_bytes) = max_bytes
        && truncated_text.len() > limit_bytes
    {
        truncated = true;
        // Floor to nearest valid char boundary <= limit_bytes
        let mut boundary = limit_bytes;
        while boundary > 0 && !truncated_text.is_char_boundary(boundary) {
            boundary = boundary.saturating_sub(1);
        }

        if let Some(slice) = truncated_text.get(..boundary) {
            truncated_text = slice.to_owned();
        }
    }

    if truncated && emit_notice {
        let notice = format!(
            "\n\n[TRUNCATION NOTICE: Output truncated. Showing {} of {} lines, {} of {} bytes]",
            truncated_text.lines().count(),
            total_lines,
            truncated_text.len(),
            total_bytes
        );
        truncated_text.push_str(&notice);
    }

    truncated_text
}

#[cfg(test)]
#[allow(
    clippy::expect_used,
    clippy::unwrap_used,
    clippy::panic,
    clippy::indexing_slicing,
    clippy::assertions_on_result_states,
    reason = "Unit test assertions"
)]
mod tests {
    use super::*;

    #[test]
    fn test_dangerous_files_rejected() {
        assert!(check_dangerous_files(Path::new(".env")).is_err());
        assert!(check_dangerous_files(Path::new("config/.env.local")).is_err());
        assert!(check_dangerous_files(Path::new("keys/id_rsa")).is_err());
        assert!(check_dangerous_files(Path::new(".git/HEAD")).is_err());
        assert!(check_dangerous_files(Path::new("src/lib.rs")).is_ok());
    }

    #[test]
    fn test_path_traversal_rejected() {
        let root = std::env::temp_dir();
        assert!(resolve_safe_path(&root, "../etc/passwd", &[], true).is_err());
        assert!(resolve_safe_path(&root, "foo/../../etc", &[], true).is_err());
    }

    #[test]
    fn test_allowed_directories_enforced() {
        let root = std::env::temp_dir();
        let allowed = vec!["tests".to_owned(), "benches".to_owned()];

        assert!(resolve_safe_path(&root, "tests/unit_test.rs", &allowed, false).is_ok());
        assert!(resolve_safe_path(&root, "crates/core/tests/test.rs", &allowed, false).is_ok());
        assert!(resolve_safe_path(&root, "src/main.rs", &allowed, false).is_err());
    }

    #[test]
    fn test_truncation() {
        let sample = "Line 1\nLine 2\nLine 3\nLine 4\nLine 5";
        let truncated = truncate_output(sample, Some(2), None, true);
        assert!(truncated.contains("Line 1\nLine 2"));
        assert!(truncated.contains("[TRUNCATION NOTICE:"));
    }
}
