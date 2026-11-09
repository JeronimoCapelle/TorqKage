//! `PreInvocation` working tree change detector.

use std::path::{Path, PathBuf};

use agy_common::{PreInvocationInput, PreInvocationResponse, WorkspaceResolver};

use crate::stdin::read_stdin_payload;

/// Executes the `detect-wip` hook.
///
/// Computes an in-memory hash of `git status -s` or `jj diff --summary`.
/// If files were modified on disk since the previous turn, emits a transient ephemeral notification.
/// If unchanged or clean, emits `{}` burning zero tokens.
pub(crate) fn execute() {
    let input: PreInvocationInput = read_stdin_payload();

    let root = resolve_workspace_root(&input);
    let config = crate::config::HooksConfig::load(&root);

    let resp = if config.wip.enabled {
        detect_wip_changes(&root, input.common.conversation_id.as_deref(), &config)
    } else {
        PreInvocationResponse::empty()
    };

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

fn detect_wip_changes(
    root: &Path,
    conversation_id: Option<&str>,
    config: &crate::config::HooksConfig,
) -> PreInvocationResponse {
    let result =
        agy_common::wip::check_wip_changes(root, conversation_id, config.wip.max_preview_files);
    result.format_notice().map_or_else(
        PreInvocationResponse::empty,
        PreInvocationResponse::ephemeral,
    )
}

#[cfg(test)]
#[allow(clippy::unwrap_used, reason = "Unit test assertions")]
mod tests {
    use super::*;

    #[test]
    fn test_detect_wip_clean() {
        let temp_dir = std::env::temp_dir();
        let config = crate::config::HooksConfig::default();
        let resp = detect_wip_changes(&temp_dir, Some("test_session"), &config);
        let json = serde_json::to_string(&resp).unwrap();
        assert_eq!(json, "{}");
    }
}
