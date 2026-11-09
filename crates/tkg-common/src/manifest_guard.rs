//! Manifest mutation guard blocking unauthorized build manifest modifications.
//!
//! Neutralizes procedural-macro / `build.rs` execution sandbox escape vectors.

use std::path::Path;

use crate::models::AgentGuard;

/// Canonical default list of protected build system manifests across all major languages.
pub const DEFAULT_PROTECTED_MANIFESTS: &[&str] = &[
    "Cargo.toml",
    "Cargo.lock",
    "build.rs",
    "package.json",
    "package-lock.json",
    "pnpm-lock.yaml",
    "yarn.lock",
    "CMakeLists.txt",
    "meson.build",
    "build.zig",
    "go.mod",
    "go.sum",
    "pyproject.toml",
    "requirements.txt",
    "setup.py",
    "pom.xml",
    "build.gradle",
    "Gemfile",
    "Makefile",
];

/// Verifies whether a given file path corresponds to a protected build manifest.
///
/// If `custom_patterns` is provided and non-empty, checks against those patterns
/// (supporting `*` prefix wildcards and exact case-insensitive matches).
/// Otherwise, checks against [`DEFAULT_PROTECTED_MANIFESTS`].
#[must_use]
pub fn is_manifest_protected(path_str: &str, custom_patterns: &[String]) -> bool {
    let p = Path::new(path_str);
    let file_name = p.file_name().and_then(|f| f.to_str()).unwrap_or("");

    if custom_patterns.is_empty() {
        return DEFAULT_PROTECTED_MANIFESTS
            .iter()
            .any(|m| m.eq_ignore_ascii_case(file_name) || m.eq_ignore_ascii_case(path_str));
    }

    custom_patterns.iter().any(|pat| {
        pat.strip_prefix('*').map_or_else(
            || file_name.eq_ignore_ascii_case(pat) || path_str.eq_ignore_ascii_case(pat),
            |suffix| file_name.ends_with(suffix) || path_str.ends_with(suffix),
        )
    })
}

/// Verifies that unauthorized agents do not mutate protected build manifests.
///
/// # Errors
/// Returns an error if the agent is not permitted to mutate protected build files.
pub fn check_manifest_protection(
    target_path: &Path,
    agent_name: Option<&str>,
    guard: Option<&AgentGuard>,
) -> Result<(), String> {
    let path_str = target_path.to_string_lossy();
    if !is_manifest_protected(&path_str, &[]) {
        return Ok(());
    }

    let file_name = target_path
        .file_name()
        .and_then(|f| f.to_str())
        .unwrap_or("");

    if let Some(g) = guard {
        if g.allowed_files.iter().any(|f| f == file_name || f == "*") {
            return Ok(());
        }
        if let Some(name) = agent_name
            && name != "coder"
            && !g.allowed_files.iter().any(|f| f == file_name)
        {
            return Err(format!(
                "Manifest protection violation: Agent '{name}' is not authorized to mutate build manifest '{file_name}'"
            ));
        }
    }

    Ok(())
}

/// Fallback heuristic verifying if a tool name implies mutation semantics.
#[must_use]
pub fn is_default_fallback_mutation_tool(name: &str) -> bool {
    let bytes = name.as_bytes();
    if matches!(
        bytes,
        b"file_write" | b"write_file" | b"mpatch_edit" | b"modify_code" | b"edit_file"
    ) {
        return true;
    }
    let lower = name.to_ascii_lowercase();
    lower.contains("write")
        || lower.contains("edit")
        || lower.contains("modify")
        || lower.contains("patch")
        || lower.contains("create")
        || lower.contains("append")
        || lower.contains("delete")
        || lower.contains("remove")
        || lower.contains("overwrite")
        || lower.contains("replace")
}

fn is_path_key(key: &str) -> bool {
    let k = key.to_ascii_lowercase();
    k.contains("path") || k.contains("file") || k == "dest" || k == "destination"
}

fn is_candidate_path(s: &str) -> bool {
    let trimmed = s.trim();
    !trimmed.is_empty() && !trimmed.contains('\n') && trimmed.len() <= 4096
}

/// Extracts candidate target file path arguments from a tool invocation payload.
///
/// Priority 1: Inspect keys matching common path or file conventions (`path`, `file`, `dest`, etc.).
/// Priority 2: If no semantic path keys are present, inspect any single-line string values in the arguments.
#[must_use]
pub fn extract_candidate_paths(args: &serde_json::Value) -> Vec<String> {
    let mut candidates = Vec::new();

    if let serde_json::Value::Object(map) = args {
        for (k, v) in map {
            if is_path_key(k)
                && let Some(s) = v.as_str()
                && is_candidate_path(s)
            {
                candidates.push(s.trim().to_owned());
            }
        }

        if candidates.is_empty() {
            for v in map.values() {
                if let Some(s) = v.as_str()
                    && is_candidate_path(s)
                {
                    candidates.push(s.trim().to_owned());
                }
            }
        }
    }

    candidates
}

#[cfg(test)]
#[allow(clippy::unwrap_used, reason = "Unit test assertions")]
mod tests {
    use serde_json::json;

    use super::*;

    #[test]
    fn test_is_manifest_protected_defaults() {
        assert!(is_manifest_protected("Cargo.toml", &[]));
        assert!(is_manifest_protected("crates/tkg-mcp/Cargo.toml", &[]));
        assert!(is_manifest_protected("package.json", &[]));
        assert!(is_manifest_protected("build.rs", &[]));
        assert!(is_manifest_protected("go.mod", &[]));
        assert!(is_manifest_protected("pyproject.toml", &[]));

        assert!(!is_manifest_protected("src/main.rs", &[]));
        assert!(!is_manifest_protected("README.md", &[]));
    }

    #[test]
    fn test_is_manifest_protected_custom() {
        let custom = vec!["*.lock".to_owned(), "custom.cfg".to_owned()];
        assert!(is_manifest_protected("yarn.lock", &custom));
        assert!(is_manifest_protected("custom.cfg", &custom));
        assert!(!is_manifest_protected("Cargo.toml", &custom));
    }

    #[test]
    fn test_check_manifest_protection() {
        let cargo_toml = Path::new("Cargo.toml");
        let src_file = Path::new("src/main.rs");

        // Normal files are not protected
        check_manifest_protection(src_file, Some("reviewer"), None).unwrap();

        // Manifest file without guard: coder is allowed
        check_manifest_protection(cargo_toml, Some("coder"), None).unwrap();

        // Manifest file with unauthorized agent: blocked
        let guard = AgentGuard {
            allowed_files: vec!["README.md".to_owned()],
            ..Default::default()
        };
        let err = check_manifest_protection(cargo_toml, Some("reviewer"), Some(&guard));
        assert!(err.is_err());
        assert!(err.unwrap_err().contains("Manifest protection violation"));

        // Manifest file with explicit allowance: allowed
        let allowed_guard = AgentGuard {
            allowed_files: vec!["Cargo.toml".to_owned()],
            ..Default::default()
        };
        check_manifest_protection(cargo_toml, Some("reviewer"), Some(&allowed_guard)).unwrap();
    }

    #[test]
    fn test_extract_candidate_paths() {
        let args = json!({
            "path": "crates/tkg-mcp/src/lib.rs",
            "content": "fn test() {}"
        });
        let paths = extract_candidate_paths(&args);
        assert_eq!(paths, vec!["crates/tkg-mcp/src/lib.rs"]);

        let fallback_args = json!({
            "target": "Cargo.toml",
            "count": 10
        });
        let paths2 = extract_candidate_paths(&fallback_args);
        assert_eq!(paths2, vec!["Cargo.toml"]);
    }

    #[test]
    fn test_is_default_fallback_mutation_tool() {
        assert!(is_default_fallback_mutation_tool("file_write"));
        assert!(is_default_fallback_mutation_tool("mpatch_edit"));
        assert!(is_default_fallback_mutation_tool("edit_code"));
        assert!(is_default_fallback_mutation_tool("patch_manifest"));
        assert!(!is_default_fallback_mutation_tool("file_read"));
        assert!(!is_default_fallback_mutation_tool("search_code"));
    }
}
