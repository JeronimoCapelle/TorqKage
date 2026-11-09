//! Workspace conflict detection for `PreToolUse` safety gate (`check-conflicts`).

use std::path::{Path, PathBuf};

use tkg_common::{
    GateCheck, GateCheckStatus, PreToolResponse, VcsConflictCheck, WorkspaceResolver,
};

/// Executes the `check-conflicts` command.
///
/// Inspects Jujutsu or Git for unresolved conflict markers. If any conflicts are detected,
/// prints a `PreToolResponse::deny` JSON payload to `stdout`. Otherwise, prints `PreToolResponse::allow`.
pub(crate) fn execute() {
    let root = WorkspaceResolver::find_root(None).unwrap_or_else(|_| PathBuf::from("."));

    let response =
        detect_conflicts(&root).map_or_else(PreToolResponse::allow, PreToolResponse::deny);
    crate::stdin::write_pre_tool_use_response(&response);
}

/// Checks whether any conflicts exist in the repository.
///
/// Delegates to `tkg_common::VcsConflictCheck` to inspect Jujutsu and Git conflict markers.
pub(crate) fn detect_conflicts(root: &Path) -> Option<String> {
    match VcsConflictCheck.execute(root) {
        GateCheckStatus::Passed => None,
        GateCheckStatus::Failed(reason) => {
            Some(format!("Execution blocked by tkg-hooks: {reason}"))
        },
        GateCheckStatus::Warning(warn) => Some(format!("Execution blocked by tkg-hooks: {warn}")),
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
