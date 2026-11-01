//! Workspace conflict detection for `PreToolUse` safety gate (`check-conflicts`).

use std::path::{Path, PathBuf};
use std::process::Command;
use agy_common::{PreToolResponse, WorkspaceResolver};

/// Executes the `check-conflicts` command.
///
/// Inspects Jujutsu or Git for unresolved conflict markers. If any conflicts are detected,
/// prints a `PreToolResponse::deny` JSON payload to `stdout`. Otherwise, prints `PreToolResponse::allow`.
pub(crate) fn execute() {
    let root = WorkspaceResolver::find_root(None).unwrap_or_else(|_| PathBuf::from("."));

    if let Some(reason) = detect_conflicts(&root) {
        let response = PreToolResponse::deny(reason);
        if let Ok(json) = serde_json::to_string(&response) {
            println!("{json}");
        }
    } else {
        let response = PreToolResponse::allow();
        if let Ok(json) = serde_json::to_string(&response) {
            println!("{json}");
        }
    }
}

/// Checks whether any conflicts exist in the repository.
///
/// Queries `jj log -r 'conflicts() & @' --no-graph` first, falling back to `git diff --check`.
pub(crate) fn detect_conflicts(root: &Path) -> Option<String> {
    // 1. Check Jujutsu conflicts in working copy (@)
    let jj_output = Command::new("jj")
        .args(["log", "-r", "conflicts() & @", "--no-graph", "-T", "change_id"])
        .current_dir(root)
        .output();

    if let Ok(out) = jj_output
        && out.status.success()
    {
        let stdout = String::from_utf8_lossy(&out.stdout).trim().to_owned();
        if !stdout.is_empty() {
            return Some(format!(
                "Execution blocked by agy-hooks: unresolved Jujutsu conflict in active commit (@: {stdout}). Resolve conflict markers before running compilation or tests."
            ));
        }
    }

    // 2. Check Git conflict markers in working tree
    let git_output = Command::new("git")
        .args(["diff", "--check"])
        .current_dir(root)
        .output();

    if let Ok(out) = git_output {
        let stderr = String::from_utf8_lossy(&out.stderr);
        let stdout = String::from_utf8_lossy(&out.stdout);
        let combined = format!("{stdout}\n{stderr}");
        if combined.contains("leftover conflict marker") || combined.contains("conflict marker") {
            return Some(
                "Execution blocked by agy-hooks: unresolved conflict markers detected in working tree (git diff --check). Please resolve all '<<<<<<<' markers before proceeding.".to_owned(),
            );
        }
    }

    None
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_detect_conflicts_clean_repo() {
        let root = WorkspaceResolver::find_root(None).unwrap_or_else(|_| PathBuf::from("."));
        // Current repo is clean, so detect_conflicts should return None
        let result = detect_conflicts(&root);
        assert!(result.is_none());
    }
}
