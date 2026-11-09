//! Session state handoff recording and wake-up continuity context loader.

use std::path::Path;

use crate::{AgyStatePaths, HandoffRecord, JujutsuWorkspace, PipelineHandoff, ProjectConfig};

/// Records a structured session handoff to persistent project state.
///
/// Discovers working copy changes via [`JujutsuWorkspace`], constructs a [`PipelineHandoff`]
/// and [`HandoffRecord`], and serializes them to `.agy/state/handoff.json` and `.agy/state/last_handoff.md`.
///
/// # Errors
/// Returns an error if directory creation or file writing fails.
pub fn record_session_handoff(
    workspace_root: &Path,
    from_agent: &str,
    target_agent: Option<&str>,
    summary: &str,
    deliverables: &[String],
    status: &str,
    max_modified_files: usize,
) -> Result<(PipelineHandoff, HandoffRecord), String> {
    let project = ProjectConfig::detect_or_load(workspace_root);
    let modified_files =
        JujutsuWorkspace::get_modified_files(workspace_root, &project, max_modified_files);

    let active_name = if from_agent.trim().is_empty() {
        "agent"
    } else {
        from_agent.trim()
    };

    let effective_status = if status.eq_ignore_ascii_case("auto") || status.is_empty() {
        if modified_files.is_empty() {
            "Clean"
        } else {
            "WIP"
        }
    } else {
        status
    };

    let pipeline_handoff = PipelineHandoff::new(
        active_name,
        target_agent.map(ToOwned::to_owned),
        summary,
        modified_files.clone(),
        deliverables.to_vec(),
        effective_status,
    );

    let paths = AgyStatePaths::new(workspace_root);
    let state_dir = paths.state_dir();
    std::fs::create_dir_all(&state_dir).map_err(|e| {
        format!(
            "Failed to create state directory '{}': {e}",
            state_dir.display()
        )
    })?;

    let json_content = serde_json::to_string_pretty(&pipeline_handoff)
        .map_err(|e| format!("Failed to serialize handoff.json: {e}"))?;
    std::fs::write(paths.handoff_json_file(), json_content)
        .map_err(|e| format!("Failed to write handoff.json: {e}"))?;

    let record = HandoffRecord::new(active_name, modified_files, effective_status);
    std::fs::write(paths.handoff_md_file(), record.to_markdown())
        .map_err(|e| format!("Failed to write last_handoff.md: {e}"))?;

    Ok((pipeline_handoff, record))
}

/// Reads the latest structured [`PipelineHandoff`] from `.agy/state/handoff.json`.
#[must_use]
pub fn read_session_handoff(workspace_root: &Path) -> Option<PipelineHandoff> {
    let paths = AgyStatePaths::new(workspace_root);
    let json_handoff = paths.handoff_json_file();
    if json_handoff.is_file()
        && let Ok(content) = std::fs::read_to_string(&json_handoff)
        && let Ok(handoff) = serde_json::from_str::<PipelineHandoff>(&content)
    {
        return Some(handoff);
    }
    None
}

/// Reads the latest human-readable [`HandoffRecord`] from `.agy/state/last_handoff.md`.
#[must_use]
pub fn read_session_handoff_record(workspace_root: &Path) -> Option<HandoffRecord> {
    let paths = AgyStatePaths::new(workspace_root);
    let md_handoff = paths.handoff_md_file();
    if md_handoff.is_file()
        && let Ok(content) = std::fs::read_to_string(&md_handoff)
        && let Some(record) = HandoffRecord::from_markdown(&content)
    {
        return Some(record);
    }
    None
}

/// Resolves an actionable wake-up injection message from the latest session handoff.
///
/// Priority 1: Reads and renders `.agy/state/handoff.json`.
/// Priority 2: Reads and parses `.agy/state/last_handoff.md`.
#[must_use]
pub fn read_last_handoff_message(workspace_root: &Path) -> Option<String> {
    if let Some(handoff) = read_session_handoff(workspace_root) {
        return Some(handoff.to_injection_message());
    }

    if let Some(record) = read_session_handoff_record(workspace_root) {
        let files_str = if record.modified_files.is_empty() {
            "Working tree clean.".to_owned()
        } else {
            format!("Modified files: {}.", record.modified_files.join(", "))
        };

        return Some(format!(
            "[WAKE-UP STATE]: Last session was with {}. {}. Status: {}.",
            record.previous_agent, files_str, record.status
        ));
    }

    None
}

#[cfg(test)]
#[allow(clippy::unwrap_used, reason = "Unit test assertions")]
mod tests {
    use super::*;

    #[test]
    fn test_record_and_read_session_handoff() {
        let temp_dir = std::env::temp_dir().join(format!(
            "common_handoff_test_{}",
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        drop(std::fs::create_dir_all(&temp_dir));

        let (handoff, record) = record_session_handoff(
            &temp_dir,
            "coder",
            Some("finisher"),
            "Implemented new handoff module",
            &["src/handoff.rs".to_owned()],
            "ReadyForReview",
            10,
        )
        .unwrap();

        assert_eq!(handoff.from_agent, "coder");
        assert_eq!(handoff.target_agent.as_deref(), Some("finisher"));
        assert_eq!(handoff.status, "ReadyForReview");
        assert_eq!(record.previous_agent, "coder");

        // Read back
        let read_back = read_session_handoff(&temp_dir).unwrap();
        assert_eq!(read_back.summary, "Implemented new handoff module");

        let msg = read_last_handoff_message(&temp_dir).unwrap();
        assert!(msg.contains("coder' to 'finisher'"));
        assert!(msg.contains("Implemented new handoff module"));

        drop(std::fs::remove_dir_all(&temp_dir));
    }

    #[test]
    fn test_read_handoff_missing_returns_none() {
        let temp_dir = std::env::temp_dir().join(format!(
            "common_handoff_missing_{}",
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        assert!(read_session_handoff(&temp_dir).is_none());
        assert!(read_last_handoff_message(&temp_dir).is_none());
    }
}
