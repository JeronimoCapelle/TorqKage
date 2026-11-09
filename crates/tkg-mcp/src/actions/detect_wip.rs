//! In-process working tree modification detector.

use std::path::Path;

/// Executes the working copy modification detector.
///
/// Discovers working tree changes and tracks cache state via canonical [`tkg_common::wip::check_wip_changes`].
/// Returns a concise status message and whether an error condition occurred (`false`).
pub(crate) fn execute(workspace_root: &Path, arguments: &serde_json::Value) -> (String, bool) {
    let conversation_id = arguments
        .get("conversation_id")
        .or_else(|| arguments.get("session_id"))
        .and_then(serde_json::Value::as_str);

    let max_preview = arguments
        .get("max_preview")
        .and_then(serde_json::Value::as_u64)
        .map_or(10, |v| usize::try_from(v).unwrap_or(10));

    let result = tkg_common::wip::check_wip_changes(workspace_root, conversation_id, max_preview);
    (result.format_status_message(), false)
}

#[cfg(test)]
#[allow(clippy::unwrap_used, reason = "Unit test assertions")]
mod tests {
    use super::*;

    #[test]
    fn test_detect_wip_clean_or_modified() {
        let temp_dir = std::env::temp_dir().join(format!(
            "mcp_wip_test_{}",
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        drop(std::fs::create_dir_all(&temp_dir));

        let (out, is_err) = execute(&temp_dir, &serde_json::json!({}));
        assert!(!is_err);
        assert!(out.contains("Working copy") || out.contains("[Notice: Files modified"));

        drop(std::fs::remove_dir_all(&temp_dir));
    }
}
