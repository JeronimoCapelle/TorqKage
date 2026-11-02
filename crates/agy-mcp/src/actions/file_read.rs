//! `file_read` action handler with path confinement and safe truncation.

use std::path::Path;
use crate::guards::{check_dangerous_files, check_enforce_boundaries, resolve_safe_path, truncate_output};
use crate::models::ToolDefinition;

/// Executes a bounded file read action.
///
/// # Errors
/// Returns an error message if the path violates security guards, does not exist, or cannot be read.
pub(crate) fn execute(
    tool: &ToolDefinition,
    workspace_root: &Path,
    arguments: &serde_json::Value,
) -> Result<String, String> {
    let path_str = arguments
        .get("path")
        .and_then(serde_json::Value::as_str)
        .ok_or_else(|| "Missing required parameter 'path'".to_owned())?;

    if tool.ban_dangerous_files() {
        check_dangerous_files(Path::new(path_str))?;
    }

    let safe_path = resolve_safe_path(
        workspace_root,
        path_str,
        &tool.allowed_directories,
        tool.enforce_workspace_bounds(),
    )?;

    check_enforce_boundaries(
        workspace_root,
        &safe_path,
        tool.enforce_dir.as_deref(),
        tool.enforce_file_target.as_deref(),
    )?;

    let raw_content = std::fs::read_to_string(&safe_path)
        .map_err(|err| format!("Failed to read file '{}': {err}", safe_path.display()))?;

    let start_line = arguments.get("start_line").and_then(serde_json::Value::as_u64);
    let end_line = arguments.get("end_line").and_then(serde_json::Value::as_u64);

    let content_to_process = if start_line.is_some() || end_line.is_some() {
        slice_lines(&raw_content, start_line, end_line)
    } else {
        raw_content
    };

    let result = truncate_output(
        &content_to_process,
        tool.truncate_lines,
        tool.truncate_bytes,
        tool.emit_truncation_notice(),
    );

    Ok(result)
}

fn slice_lines(text: &str, start_opt: Option<u64>, end_opt: Option<u64>) -> String {
    let start_raw = start_opt.unwrap_or(1);
    let start = usize::try_from(start_raw).unwrap_or(1).saturating_sub(1);
    let end = end_opt.map_or(usize::MAX, |e| usize::try_from(e).unwrap_or(usize::MAX));

    let mut selected = Vec::new();
    for (idx, line) in text.lines().enumerate() {
        let line_num = idx.saturating_add(1);
        if line_num >= start.saturating_add(1) && line_num <= end {
            selected.push(line);
        }
    }

    selected.join("\n")
}

#[cfg(test)]
#[allow(
    clippy::expect_used,
    clippy::unwrap_used,
    clippy::panic,
    clippy::indexing_slicing,
    reason = "Unit test assertions"
)]
mod tests {
    use super::*;

    #[test]
    fn test_slice_lines() {
        let content = "Line 1\nLine 2\nLine 3\nLine 4\nLine 5";
        let sliced = slice_lines(content, Some(2), Some(4));
        assert_eq!(sliced, "Line 2\nLine 3\nLine 4");
    }
}
