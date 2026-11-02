//! Workspace conflict detection for `PreToolUse` safety gate (`check-conflicts`).

use std::path::{Path, PathBuf};
use agy_common::{GateCheck, GateCheckStatus, PreToolResponse, VcsConflictCheck, WorkspaceResolver};

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
/// Delegates to `agy_common::VcsConflictCheck` to inspect Jujutsu and Git conflict markers.
pub(crate) fn detect_conflicts(root: &Path) -> Option<String> {
    match VcsConflictCheck.execute(root) {
        GateCheckStatus::Passed => None,
        GateCheckStatus::Failed(reason) => Some(format!("Execution blocked by agy-hooks: {reason}")),
        GateCheckStatus::Warning(warn) => Some(format!("Execution blocked by agy-hooks: {warn}")),
    }
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
