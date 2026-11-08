//! `file_read` action handler with path confinement and safe truncation.

use std::{collections::BTreeMap, path::Path};

use agy_common::ProjectConfig;

use crate::{
    guards::{
        check_dangerous_files_with_security, check_enforce_boundaries, resolve_safe_path,
        resolve_scoped_path_str, truncate_output,
    },
    models::{AgentGuard, ToolDefinition},
};

/// Executes a bounded file read action.
///
/// # Errors
/// Returns an error message if the path violates security guards, does not exist, or cannot be read.
pub(crate) fn execute(
    tool: &ToolDefinition,
    workspace_root: &Path,
    arguments: &serde_json::Value,
    agent_name: Option<&str>,
    agent_guards: &BTreeMap<String, AgentGuard>,
) -> Result<String, String> {
    let raw_path = arguments
        .get("path")
        .and_then(serde_json::Value::as_str)
        .ok_or_else(|| "Missing required parameter 'path'".to_owned())?;

    let resolved_path = resolve_scoped_path_str(
        workspace_root,
        raw_path,
        Some(tool),
        agent_name.map(|name| (name, agent_guards)),
    );

    if tool.ban_dangerous_files() {
        let project = ProjectConfig::detect_or_load(workspace_root);
        check_dangerous_files_with_security(Path::new(&resolved_path), Some(&project.security))?;
    }

    let safe_path = resolve_safe_path(
        workspace_root,
        &resolved_path,
        &tool.allowed_directories,
        tool.enforce_workspace_bounds(),
    )?;

    check_enforce_boundaries(
        workspace_root,
        &safe_path,
        tool.enforce_dir.as_deref(),
        tool.enforce_file_target.as_deref(),
    )?;

    if safe_path.is_dir() {
        return Err(format!(
            "Cannot read '{}': path is a directory, not a regular file",
            safe_path.display()
        ));
    }

    let raw_content = std::fs::read_to_string(&safe_path)
        .map_err(|err| format!("Failed to read file '{}': {err}", safe_path.display()))?;

    let start_line = arguments
        .get("start_line")
        .and_then(serde_json::Value::as_u64);
    let end_line = arguments
        .get("end_line")
        .and_then(serde_json::Value::as_u64);

    if let (Some(start), Some(end)) = (start_line, end_line)
        && start > end
    {
        return Err(format!(
            "Invalid line range: start_line ({start}) cannot be greater than end_line ({end})"
        ));
    }

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

    #[test]
    fn test_file_read_inverted_bounds_error() {
        let temp = std::env::temp_dir().join(format!("agy_read_bounds_{}", std::process::id()));
        drop(std::fs::remove_dir_all(&temp));
        std::fs::create_dir_all(&temp).expect("create dir");
        let file_path = temp.join("test.txt");
        std::fs::write(&file_path, "Line 1\nLine 2\n").expect("write test file");

        let tool = ToolDefinition {
            name: "file_read".to_owned(),
            ..Default::default()
        };
        let guards = BTreeMap::new();
        let args = serde_json::json!({
            "path": file_path.to_str().unwrap(),
            "start_line": 10,
            "end_line": 5
        });

        let res = execute(&tool, &temp, &args, None, &guards);
        assert!(res.is_err());
        assert!(res.unwrap_err().contains("Invalid line range"));
    }

    #[test]
    fn test_file_read_scoped_path_resolution() {
        let temp = std::env::temp_dir().join(format!("agy_read_scoped_{}", std::process::id()));
        drop(std::fs::remove_dir_all(&temp));
        std::fs::create_dir_all(temp.join("src/auth")).expect("create src/auth");
        std::fs::create_dir_all(temp.join("docs/specs")).expect("create docs/specs");
        std::fs::write(temp.join("src/auth/hasher.rs"), "pub fn hash() {}").expect("write hasher");
        std::fs::write(temp.join("Cargo.toml"), "[package]\nname = \"demo\"\n")
            .expect("write Cargo.toml");
        std::fs::write(temp.join("docs/specs/rv32.md"), "# RV32 Spec").expect("write spec");

        let mut guards = BTreeMap::new();
        guards.insert(
            "worker_agent".to_owned(),
            AgentGuard {
                allowed_dirs: vec!["src".to_owned()],
                ..Default::default()
            },
        );

        let default_tool = ToolDefinition {
            name: "read_file".to_owned(),
            allowed_directories: vec!["src".to_owned()],
            ..Default::default()
        };

        // 1. Worker reads scoped relative path "auth/hasher.rs" -> resolves to "src/auth/hasher.rs"
        let args = serde_json::json!({ "path": "auth/hasher.rs" });
        let res = execute(&default_tool, &temp, &args, Some("worker_agent"), &guards)
            .expect("read auth/hasher.rs");
        assert!(res.contains("pub fn hash() {}"));

        // 2. Read root Cargo.toml (with tool allowing workspace root)
        let root_tool = ToolDefinition {
            name: "read_file".to_owned(),
            ..Default::default()
        };
        let root_args = serde_json::json!({ "path": "Cargo.toml" });
        let res_root = execute(&root_tool, &temp, &root_args, Some("worker_agent"), &guards)
            .expect("read Cargo.toml");
        assert!(res_root.contains("name = \"demo\""));

        // 3. Tool with enforce_dir "docs" reading "specs/rv32.md" -> resolves to "docs/specs/rv32.md"
        let docs_tool = ToolDefinition {
            name: "read_doc".to_owned(),
            enforce_dir: Some("docs".to_owned()),
            ..Default::default()
        };
        let doc_args = serde_json::json!({ "path": "specs/rv32.md" });
        let res_doc = execute(&docs_tool, &temp, &doc_args, None, &guards).expect("read doc");
        assert!(res_doc.contains("# RV32 Spec"));

        // 4. Reading a directory produces clean actionable error without failing with raw OS error 21
        let dir_path = temp.join("sub_dir");
        std::fs::create_dir_all(&dir_path).unwrap();
        let dir_args = serde_json::json!({ "path": "sub_dir" });
        let dir_err = execute(&root_tool, &temp, &dir_args, None, &guards).unwrap_err();
        assert!(dir_err.contains("is a directory, not a regular file"));

        drop(std::fs::remove_dir_all(&temp));
    }
}
