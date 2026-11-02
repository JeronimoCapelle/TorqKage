//! `PreInvocation` working tree change detector.

use std::hash::{Hash, Hasher};
use std::path::{Path, PathBuf};
use std::process::Command;
use agy_common::{PreInvocationInput, PreInvocationResponse, WorkspaceResolver};
use crate::stdin::read_stdin_payload;

/// Executes the `detect-wip` hook.
///
/// Computes an in-memory hash of `git status -s`.
/// If files were modified on disk since the previous turn, emits a transient ephemeral notification.
/// If unchanged or clean, emits `{}` burning zero tokens.
pub(crate) fn execute() {
    let input: PreInvocationInput = read_stdin_payload();

    let root = resolve_workspace_root(&input);

    let resp = detect_wip_changes(&root, input.common.conversation_id.as_deref());

    if let Ok(json) = serde_json::to_string(&resp) {
        println!("{json}");
    }
}

fn resolve_workspace_root(input: &PreInvocationInput) -> PathBuf {
    if let Some(first_path) = input.common.workspace_paths.first()
        && first_path.exists()
    {
        return first_path.clone();
    }

    WorkspaceResolver::find_root(None).unwrap_or_else(|_| PathBuf::from("."))
}

fn detect_wip_changes(root: &Path, conversation_id: Option<&str>) -> PreInvocationResponse {
    let status_str = if agy_common::JujutsuWorkspace::is_jj_repo(root) {
        let jj_status = Command::new("jj")
            .args(["--color=never", "diff", "--summary"])
            .current_dir(root)
            .output();

        match jj_status {
            Ok(output) if output.status.success() => {
                let s = String::from_utf8_lossy(&output.stdout);
                let filtered: Vec<&str> = s
                    .lines()
                    .filter(|l| {
                        let t = l.trim();
                        !t.is_empty() && t != "A target" && t != "M target"
                    })
                    .collect();
                filtered.join("\n")
            }
            _ => String::new(),
        }
    } else {
        let git_status = Command::new("git")
            .args(["status", "-s"])
            .current_dir(root)
            .output();

        match git_status {
            Ok(output) if output.status.success() => String::from_utf8_lossy(&output.stdout).into_owned(),
            _ => String::new(),
        }
    };

    let trimmed = status_str.trim();
    if trimmed.is_empty() {
        return PreInvocationResponse::empty();
    }

    let mut hasher = std::collections::hash_map::DefaultHasher::new();
    trimmed.hash(&mut hasher);
    let current_hash = hasher.finish();

    let cache_file = get_cache_path(conversation_id);

    if let Ok(cached) = std::fs::read_to_string(&cache_file)
        && cached.trim() == format!("{current_hash}")
    {
        // Unchanged since last invocation
        return PreInvocationResponse::empty();
    }

    // Hash changed: record new hash
    drop(std::fs::write(&cache_file, format!("{current_hash}")));

    let modified_files = extract_modified_files(trimmed);
    let notice = format!("[Notice: Files modified on disk: {modified_files}]");
    PreInvocationResponse::ephemeral(notice)
}

fn get_cache_path(conversation_id: Option<&str>) -> PathBuf {
    let filename = conversation_id.map_or_else(
        || ".agy_wip_default.hash".to_owned(),
        |id| {
            let sanitized: String = id
                .chars()
                .filter(|c| c.is_alphanumeric() || *c == '-' || *c == '_')
                .collect();
            format!(".agy_wip_{sanitized}.hash")
        },
    );

    std::env::temp_dir().join(filename)
}

fn extract_modified_files(status_output: &str) -> String {
    let mut files = Vec::new();
    for line in status_output.lines().take(4) {
        let trimmed = line.trim();
        // Format: 'XY path' -> extract path
        if let Some((_, path)) = trimmed.split_once(' ') {
            files.push(path.trim());
        } else {
            files.push(trimmed);
        }
    }

    if files.is_empty() {
        "Working copy modified".to_owned()
    } else {
        files.join(", ")
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_extract_modified_files() {
        let sample = " M src/lib.rs\n?? tests/test.rs\n M Cargo.toml";
        let extracted = extract_modified_files(sample);
        assert_eq!(extracted, "src/lib.rs, tests/test.rs, Cargo.toml");
    }

    #[test]
    fn test_cache_path_generation() {
        let path = get_cache_path(Some("test-conv-123"));
        assert!(path.to_string_lossy().contains(".agy_wip_test-conv-123.hash"));
    }
}
