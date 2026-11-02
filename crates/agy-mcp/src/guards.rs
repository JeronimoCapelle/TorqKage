//! Composable security guards, path confinement, and UTF-8 truncation primitives.

use std::collections::BTreeMap;
use std::path::{Component, Path, PathBuf};
use crate::models::AgentGuard;

/// Inspects a target path against dangerous files, secrets, and repository internals.
///
/// # Errors
/// Returns an error if the path attempts to access sensitive files, credentials, or VCS internals (`.git/`, `.jj/`).
pub(crate) fn check_dangerous_files(path: &Path) -> Result<(), String> {
    let lossy = path.to_string_lossy().to_lowercase();

    // Check for sensitive filenames and patterns
    let dangerous_patterns = [
        "id_rsa",
        "id_ed25519",
        "id_ecdsa",
        ".bashrc",
        ".zshrc",
        ".profile",
    ];

    for pat in dangerous_patterns {
        if lossy.contains(pat) {
            return Err(format!(
                "Security policy violation: Access to sensitive file pattern '{pat}' is blocked."
            ));
        }
    }

    // Inspect individual path components and file names for credentials and VCS internal directories
    for comp in path.components() {
        let name = comp.as_os_str().to_string_lossy().to_lowercase();
        let p = Path::new(&name);
        let has_sensitive_ext = p
            .extension()
            .is_some_and(|ext| ext.eq_ignore_ascii_case("key") || ext.eq_ignore_ascii_case("pem"));

        if name == ".git"
            || name == ".jj"
            || name == ".env"
            || name.starts_with(".env.")
            || name == "secret"
            || name == "secrets"
            || name == "credential"
            || name == "credentials"
            || name == "password"
            || name == "passwords"
            || has_sensitive_ext
        {
            return Err(format!(
                "Security policy violation: Access to sensitive file pattern '{name}' is blocked."
            ));
        }
    }

    Ok(())
}

/// Helper verifying if a relative path is scoped within a directory name at root or crate level.
fn matches_directory_scope(rel_path: &str, target_dir: &str) -> bool {
    let clean = target_dir.trim_matches(['/', '\\']);
    if clean == "*" {
        return true;
    }

    // 1. Root-level match: e.g. "src/lib.rs" matches "src"
    if rel_path == clean
        || rel_path.starts_with(&format!("{clean}/"))
        || rel_path.starts_with(&format!("{clean}\\"))
    {
        return true;
    }

    // 2. Cargo workspace crate-level match: e.g. "crates/agy-mcp/src/main.rs" matches "src"
    for prefix in &["crates/", "packages/", "crates\\", "packages\\"] {
        if let Some(sub) = rel_path.strip_prefix(prefix)
            && let Some((_crate_name, crate_rel)) = sub.split_once(['/', '\\'])
            && (crate_rel == clean
                || crate_rel.starts_with(&format!("{clean}/"))
                || crate_rel.starts_with(&format!("{clean}\\")))
        {
            return true;
        }
    }

    false
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
            matches_directory_scope(&rel_lossy, dir)
        });

        if !allowed {
            return Err(format!(
                "Security policy violation: Path '{rel_lossy}' is outside allowed directories: {allowed_directories:?}"
            ));
        }
    }

    Ok(candidate)
}

/// Enforces single-directory confinement (`enforce_dir`) and single-file target locking (`enforce_file_target`).
///
/// # Errors
/// Returns an error if the candidate path is outside `enforce_dir` or does not match `enforce_file_target`.
pub(crate) fn check_enforce_boundaries(
    workspace_root: &Path,
    candidate: &Path,
    enforce_dir: Option<&str>,
    enforce_file_target: Option<&str>,
) -> Result<(), String> {
    let relative = candidate
        .strip_prefix(workspace_root)
        .unwrap_or(candidate);
    let rel_lossy = relative.to_string_lossy();

    // 1. Enforce single-file target locking
    if let Some(target) = enforce_file_target {
        let expected_rel = Path::new(target)
            .strip_prefix(workspace_root)
            .unwrap_or_else(|_| Path::new(target));
        let expected_lossy = expected_rel.to_string_lossy();

        if rel_lossy != expected_lossy {
            return Err(format!(
                "Security policy violation: Tool is strictly locked to target file '{expected_lossy}', but '{rel_lossy}' was requested."
            ));
        }
    }

    // 2. Enforce single-directory confinement
    if let Some(dir) = enforce_dir
        && !matches_directory_scope(&rel_lossy, dir)
    {
        return Err(format!(
            "Security policy violation: Path '{rel_lossy}' is outside enforced directory '{dir}'"
        ));
    }

    Ok(())
}

/// Enforces agent-specific directory and file path boundaries.
///
/// # Errors
/// Returns an error if the agent is restricted and the target path violates the agent's guard.
pub(crate) fn check_agent_guard(
    workspace_root: &Path,
    candidate: &Path,
    agent_name: Option<&str>,
    guards: &BTreeMap<String, AgentGuard>,
) -> Result<(), String> {
    let Some(agent) = agent_name else {
        return Ok(());
    };

    let Some(guard) = guards.get(agent) else {
        return Ok(());
    };

    let relative = candidate
        .strip_prefix(workspace_root)
        .unwrap_or(candidate);
    let rel_lossy = relative.to_string_lossy();

    // 1. Check disallowed files
    for disallowed_file in &guard.disallowed_files {
        if rel_lossy == *disallowed_file {
            return Err(format!(
                "Security policy violation: Agent '{agent}' is not permitted to modify file '{rel_lossy}'."
            ));
        }
    }

    // 2. Check disallowed directories
    for disallowed_dir in &guard.disallowed_dirs {
        if matches_directory_scope(&rel_lossy, disallowed_dir) {
            return Err(format!(
                "Security policy violation: Agent '{agent}' is prohibited from modifying files in '{disallowed_dir}/'."
            ));
        }
    }

    // 3. Check allowed files
    let matches_allowed_file = guard.allowed_files.iter().any(|f| rel_lossy == *f);
    if matches_allowed_file {
        return Ok(());
    }

    // 4. Check allowed directories
    if !guard.allowed_dirs.is_empty() {
        if guard.allowed_dirs.iter().any(|d| d == "*") {
            return Ok(());
        }

        let is_allowed = guard.allowed_dirs.iter().any(|dir| {
            matches_directory_scope(&rel_lossy, dir)
        });

        if !is_allowed {
            return Err(format!(
                "Security policy violation: Agent '{agent}' is restricted to directories {:?} or files {:?}, but '{rel_lossy}' was requested.",
                guard.allowed_dirs, guard.allowed_files
            ));
        }
    }

    Ok(())
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
        assert!(check_dangerous_files(Path::new(".jj/working_copy")).is_err());
        assert!(check_dangerous_files(Path::new("secrets/private.pem")).is_err());
        assert!(check_dangerous_files(Path::new("src/lib.rs")).is_ok());
        assert!(check_dangerous_files(Path::new("src/tokenizer.rs")).is_ok());
        assert!(check_dangerous_files(Path::new("src/tokens.rs")).is_ok());
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
        assert!(resolve_safe_path(&root, "docs/fixtures/tests/sample.rs", &allowed, false).is_err());
    }

    #[test]
    fn test_truncation() {
        let sample = "Line 1\nLine 2\nLine 3\nLine 4\nLine 5";
        let truncated = truncate_output(sample, Some(2), None, true);
        assert!(truncated.contains("Line 1\nLine 2"));
        assert!(truncated.contains("[TRUNCATION NOTICE:"));
    }

    #[test]
    fn test_enforce_boundaries() {
        let root = std::env::temp_dir();

        // Enforce file target
        let good_file = root.join("ARCHITECTURE.md");
        let bad_file = root.join("src/lib.rs");
        assert!(check_enforce_boundaries(&root, &good_file, None, Some("ARCHITECTURE.md")).is_ok());
        assert!(check_enforce_boundaries(&root, &bad_file, None, Some("ARCHITECTURE.md")).is_err());

        // Enforce dir
        let good_dir = root.join("docs/specs/rv32.md");
        let bad_dir = root.join("src/main.rs");
        assert!(check_enforce_boundaries(&root, &good_dir, Some("docs"), None).is_ok());
        assert!(check_enforce_boundaries(&root, &bad_dir, Some("docs"), None).is_err());
    }

    #[test]
    fn test_check_agent_guard() {
        let root = std::env::temp_dir();
        let mut guards = BTreeMap::new();

        guards.insert(
            "coder".to_owned(),
            AgentGuard {
                allowed_dirs: vec!["src".to_owned()],
                disallowed_dirs: vec!["tests".to_owned()],
                allowed_files: Vec::new(),
                disallowed_files: vec!["Cargo.toml".to_owned()],
            },
        );

        guards.insert(
            "planner".to_owned(),
            AgentGuard {
                allowed_dirs: vec!["docs".to_owned()],
                disallowed_dirs: vec!["src".to_owned()],
                allowed_files: vec!["ARCHITECTURE.md".to_owned()],
                disallowed_files: Vec::new(),
            },
        );

        // Coder tests
        assert!(check_agent_guard(&root, &root.join("src/lib.rs"), Some("coder"), &guards).is_ok());
        assert!(check_agent_guard(&root, &root.join("tests/test.rs"), Some("coder"), &guards).is_err());
        assert!(check_agent_guard(&root, &root.join("Cargo.toml"), Some("coder"), &guards).is_err());

        // Planner tests
        assert!(check_agent_guard(&root, &root.join("ARCHITECTURE.md"), Some("planner"), &guards).is_ok());
        assert!(check_agent_guard(&root, &root.join("docs/rfc.md"), Some("planner"), &guards).is_ok());
        assert!(check_agent_guard(&root, &root.join("src/lib.rs"), Some("planner"), &guards).is_err());

        // Unrestricted agent
        assert!(check_agent_guard(&root, &root.join("src/lib.rs"), Some("copilot"), &guards).is_ok());
        assert!(check_agent_guard(&root, &root.join("tests/test.rs"), None, &guards).is_ok());
    }
}
