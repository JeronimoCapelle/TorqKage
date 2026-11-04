//! `file_write` action handler with path confinement, directory allowlisting, and security guards.

use std::collections::BTreeMap;
use std::path::Path;
use agy_common::ProjectConfig;
use crate::guards::{
    check_agent_guard, check_dangerous_files_with_security, check_enforce_boundaries, resolve_safe_path,
    resolve_scoped_path_str,
};
use crate::models::{AgentGuard, ToolDefinition};

/// Executes a bounded file write action.
///
/// # Errors
/// Returns an error if the path violates security guards, escapes bounds, or fails filesystem write.
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

    let path_str = resolve_scoped_path_str(
        workspace_root,
        raw_path,
        Some(tool),
        agent_name.map(|name| (name, agent_guards)),
    );

    let content = arguments
        .get("content")
        .and_then(serde_json::Value::as_str)
        .ok_or_else(|| "Missing required parameter 'content'".to_owned())?;

    if tool.ban_dangerous_files() {
        let project = ProjectConfig::detect_or_load(workspace_root);
        check_dangerous_files_with_security(Path::new(&path_str), Some(&project.security))?;
    }

    let safe_path = resolve_safe_path(
        workspace_root,
        &path_str,
        &tool.allowed_directories,
        tool.enforce_workspace_bounds(),
    )?;

    check_enforce_boundaries(
        workspace_root,
        &safe_path,
        tool.enforce_dir.as_deref(),
        tool.enforce_file_target.as_deref(),
    )?;

    check_agent_guard(workspace_root, &safe_path, agent_name, agent_guards)?;

    if let Some(parent) = safe_path.parent()
        && !parent.exists()
    {
        std::fs::create_dir_all(parent)
            .map_err(|err| format!("Failed to create parent directory '{}': {err}", parent.display()))?;
    }

    std::fs::write(&safe_path, content.as_bytes())
        .map_err(|err| format!("Failed to write file '{}': {err}", safe_path.display()))?;

    let bytes = content.len();
    let lines = content.lines().count();

    Ok(format!(
        "Successfully wrote {bytes} bytes ({lines} lines) to '{path_str}'."
    ))
}

#[cfg(test)]
#[allow(
    clippy::expect_used,
    clippy::unwrap_used,
    clippy::panic,
    clippy::indexing_slicing,
    clippy::assertions_on_result_states,
    reason = "Unit test assertions"
)]
mod tests {
    use super::*;
    use serde_json::json;

    fn create_test_dir(tag: &str) -> std::path::PathBuf {
        let dir = std::env::temp_dir().join(format!(
            "agy-test-write-{tag}-{}",
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        std::fs::create_dir_all(&dir).expect("create test dir");
        dir
    }

    #[test]
    fn test_file_write_success() {
        let temp = create_test_dir("success");
        let tool = ToolDefinition {
            name: "write_file".to_owned(),
            allowed_directories: vec!["src".to_owned()],
            sandbox: crate::models::ToolSandboxPolicy {
                enforce_workspace_bounds: true,
                ban_dangerous_files: true,
                ..Default::default()
            },
            ..Default::default()
        };

        let args = json!({
            "path": "src/lib.rs",
            "content": "pub fn hello() -> &'static str { \"world\" }\n"
        });

        let guards = BTreeMap::new();
        let res = execute(&tool, &temp, &args, None, &guards);
        assert!(res.is_ok());

        let written = std::fs::read_to_string(temp.join("src/lib.rs")).unwrap();
        assert!(written.contains("hello()"));
    }

    #[test]
    fn test_file_write_dangerous_file_blocked() {
        let temp = create_test_dir("danger");
        let tool = ToolDefinition {
            name: "write_file".to_owned(),
            sandbox: crate::models::ToolSandboxPolicy {
                ban_dangerous_files: true,
                ..Default::default()
            },
            ..Default::default()
        };

        let args = json!({
            "path": ".env",
            "content": "SECRET=1234\n"
        });

        let guards = BTreeMap::new();
        let res = execute(&tool, &temp, &args, None, &guards);
        assert!(res.is_err());
        assert!(res.unwrap_err().contains("sensitive file pattern"));
    }

    #[test]
    fn test_file_write_disallowed_directory_blocked() {
        let temp = create_test_dir("disallow");
        let tool = ToolDefinition {
            name: "write_file".to_owned(),
            allowed_directories: vec!["src".to_owned()],
            ..Default::default()
        };

        let args = json!({
            "path": "tests/test.rs",
            "content": "fn test() {}"
        });

        let guards = BTreeMap::new();
        let res = execute(&tool, &temp, &args, None, &guards);
        assert!(res.is_err());
        assert!(res.unwrap_err().contains("outside allowed directories"));
    }

    #[test]
    fn test_file_write_enforce_file_target_and_dir() {
        let temp = create_test_dir("enforce");
        let tool = ToolDefinition {
            name: "write_spec".to_owned(),
            enforce_file_target: Some(".agy/artifacts/spec.md".to_owned()),
            ..Default::default()
        };

        let bad_args = json!({
            "path": ".agy/artifacts/other.md",
            "content": "# Other"
        });
        let guards = BTreeMap::new();
        assert!(execute(&tool, &temp, &bad_args, None, &guards).is_err());

        let good_args = json!({
            "path": ".agy/artifacts/spec.md",
            "content": "# Spec"
        });
        assert!(execute(&tool, &temp, &good_args, None, &guards).is_ok());
    }

    #[test]
    fn test_file_write_agent_guard_enforced() {
        let temp = create_test_dir("agent-guard");
        let tool = ToolDefinition {
            name: "write_file".to_owned(),
            sandbox: crate::models::ToolSandboxPolicy {
                enforce_workspace_bounds: true,
                ..Default::default()
            },
            ..Default::default()
        };

        let mut guards = BTreeMap::new();
        guards.insert(
            "coder".to_owned(),
            AgentGuard {
                allowed_dirs: vec!["src".to_owned()],
                ..Default::default()
            },
        );

        let test_file = json!({
            "path": "tests/test.rs",
            "content": "// test"
        });
        assert!(execute(&tool, &temp, &test_file, Some("coder"), &guards).is_err());

        let src_file = json!({
            "path": "src/main.rs",
            "content": "fn main() {}"
        });
        assert!(execute(&tool, &temp, &src_file, Some("coder"), &guards).is_ok());
    }

    #[test]
    fn test_file_write_scoped_resolution_creates_in_scope() {
        let temp = create_test_dir("write-scope");
        let tool = ToolDefinition {
            name: "write_file".to_owned(),
            allowed_directories: vec!["src".to_owned()],
            ..Default::default()
        };

        let mut guards = BTreeMap::new();
        guards.insert(
            "coder".to_owned(),
            AgentGuard {
                allowed_dirs: vec!["src".to_owned()],
                ..Default::default()
            },
        );

        // 1. Coder creates new file "auth/jwt.rs" without prefix -> created in src/auth/jwt.rs
        let args = json!({
            "path": "auth/jwt.rs",
            "content": "pub fn verify() -> bool { true }"
        });
        assert!(execute(&tool, &temp, &args, Some("coder"), &guards).is_ok());
        assert!(temp.join("src/auth/jwt.rs").exists());
        let content = std::fs::read_to_string(temp.join("src/auth/jwt.rs")).expect("read created file");
        assert!(content.contains("pub fn verify()"));

        // 2. Tool with enforce_dir "docs" creates "rfcs/001.md" -> created in docs/rfcs/001.md
        let docs_tool = ToolDefinition {
            name: "write_doc".to_owned(),
            enforce_dir: Some("docs".to_owned()),
            ..Default::default()
        };
        let doc_args = json!({
            "path": "rfcs/001.md",
            "content": "# RFC 001"
        });
        assert!(execute(&docs_tool, &temp, &doc_args, None, &guards).is_ok());
        assert!(temp.join("docs/rfcs/001.md").exists());
    }
}
