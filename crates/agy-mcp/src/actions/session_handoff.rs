//! Action handler for recording structured session handoffs.

use std::{fmt::Write, path::Path};

/// Executes the session handoff recording action.
///
/// Discovers working copy modifications and agent deliverables, delegating persistence
/// to [`agy_common::handoff::record_session_handoff`].
///
/// # Errors
/// Returns an error if required arguments are missing or saving fails.
pub(crate) fn execute(
    workspace_root: &Path,
    arguments: &serde_json::Value,
    from_agent: Option<&str>,
) -> Result<(String, bool), String> {
    let summary = arguments
        .get("summary")
        .and_then(serde_json::Value::as_str)
        .ok_or_else(|| "Missing required parameter 'summary'".to_owned())?;

    let status = arguments
        .get("status")
        .and_then(serde_json::Value::as_str)
        .unwrap_or("ReadyForNextPhase");

    let target_agent = arguments
        .get("target_agent")
        .and_then(serde_json::Value::as_str);

    let deliverables: Vec<String> = match arguments.get("deliverables") {
        Some(serde_json::Value::Array(arr)) => arr
            .iter()
            .filter_map(serde_json::Value::as_str)
            .map(ToOwned::to_owned)
            .collect(),
        Some(serde_json::Value::String(s)) => s
            .split(',')
            .map(str::trim)
            .filter(|item| !item.is_empty())
            .map(ToOwned::to_owned)
            .collect(),
        _ => Vec::new(),
    };

    let active_name = from_agent.unwrap_or("agent");

    let (pipeline_handoff, _) = agy_common::handoff::record_session_handoff(
        workspace_root,
        active_name,
        target_agent,
        summary,
        &deliverables,
        status,
        15,
    )?;

    let target_hint = target_agent.map_or_else(String::new, |t| format!(" -> '{t}'"));
    let mut out = format!(
        "✔ Session handoff successfully recorded{target_hint}\n- Status: {status}\n- Summary: {summary}\n"
    );
    if !pipeline_handoff.modified_files.is_empty() {
        _ = writeln!(
            out,
            "- Modified Files: {}",
            pipeline_handoff.modified_files.join(", ")
        );
    }
    if !pipeline_handoff.deliverables.is_empty() {
        _ = writeln!(
            out,
            "- Deliverables: {}",
            pipeline_handoff.deliverables.join(", ")
        );
    }

    Ok((out, false))
}

#[cfg(test)]
#[allow(clippy::unwrap_used, reason = "Unit test assertions")]
mod tests {
    use agy_common::{AgyStatePaths, PipelineHandoff};

    use super::*;

    #[test]
    fn test_record_session_handoff_success() {
        let temp_dir = std::env::temp_dir().join(format!(
            "mcp_handoff_test_{}",
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        drop(std::fs::remove_dir_all(&temp_dir));
        drop(std::fs::create_dir_all(&temp_dir));

        let args = serde_json::json!({
            "summary": "Completed parser implementation",
            "status": "ReadyForReview",
            "target_agent": "finisher",
            "deliverables": "src/parser.rs, tests/parser_tests.rs"
        });

        let (out, is_err) = execute(&temp_dir, &args, Some("coder")).unwrap();
        assert!(!is_err);
        assert!(out.contains("Session handoff successfully recorded"));
        assert!(out.contains("ReadyForReview"));

        let paths = AgyStatePaths::new(&temp_dir);
        assert!(paths.handoff_json_file().is_file());
        assert!(paths.handoff_md_file().is_file());

        let json_str = std::fs::read_to_string(paths.handoff_json_file()).unwrap();
        let handoff: PipelineHandoff = serde_json::from_str(&json_str).unwrap();
        assert_eq!(handoff.from_agent, "coder");
        assert_eq!(handoff.target_agent.as_deref(), Some("finisher"));
        assert_eq!(handoff.summary, "Completed parser implementation");
        assert_eq!(handoff.status, "ReadyForReview");
        assert_eq!(
            handoff.deliverables,
            vec!["src/parser.rs", "tests/parser_tests.rs"]
        );

        drop(std::fs::remove_dir_all(&temp_dir));
    }
}
