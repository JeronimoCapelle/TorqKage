//! `mpatch_edit` action handler applying fuzzy Aider search/replace blocks and unified diffs.

use std::{collections::BTreeMap, path::Path};

use agy_common::ProjectConfig;
use mpatch::{ApplyOptions, patch_content_str};

use crate::{
    guards::{
        check_agent_guard, check_agent_path_policies, check_dangerous_files_with_security,
        check_enforce_boundaries, resolve_safe_path, resolve_scoped_path_str,
    },
    models::{AgentGuard, ToolDefinition},
};

/// Executes a bounded fuzzy code modification using `mpatch`.
///
/// # Errors
/// Returns an error message if guards fail, the file cannot be read, or the patch fails to apply.
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

    let patch = arguments
        .get("patch")
        .and_then(serde_json::Value::as_str)
        .ok_or_else(|| "Missing required parameter 'patch'".to_owned())?;

    let project = ProjectConfig::detect_or_load(workspace_root);

    if tool.ban_dangerous_files() {
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

    if !safe_path.exists() {
        return Err(format!(
            "Target file '{path_str}' does not exist. Use a file creation tool instead."
        ));
    }

    if safe_path.is_dir() {
        return Err(format!(
            "Cannot modify '{}': target path is a directory, not a file",
            safe_path.display()
        ));
    }

    let original = std::fs::read_to_string(&safe_path).map_err(|err| {
        format!(
            "Failed to read target file '{}': {err}",
            safe_path.display()
        )
    })?;

    let options = ApplyOptions::new();

    // 1. Try applying patch directly via mpatch
    let patched_res = patch_content_str(patch, Some(&original), &options);

    // 2. If mpatch needed a file path prefix for Aider block, try with path header prepended
    let new_content = if let Ok(patched) = patched_res {
        patched
    } else {
        let prefixed = format!("{path_str}\n{patch}");
        if let Ok(patched) = patch_content_str(&prefixed, Some(&original), &options) {
            patched
        } else if let Some(fallback_content) = apply_exact_aider_block(&original, patch) {
            fallback_content
        } else {
            return Err(format!(
                "Failed to apply patch to '{path_str}'. Verify search block matches existing file content."
            ));
        }
    };

    if new_content == original {
        return Ok(format!(
            "Patch applied to '{path_str}' with 0 net changes (identical content)."
        ));
    }

    check_agent_path_policies(
        workspace_root,
        &safe_path,
        &original,
        &new_content,
        agent_name,
        agent_guards,
        Some(&project),
    )?;

    std::fs::write(&safe_path, new_content.as_bytes()).map_err(|err| {
        format!(
            "Failed to write patched file '{}': {err}",
            safe_path.display()
        )
    })?;

    let lines_before = original.lines().count();
    let lines_after = new_content.lines().count();

    Ok(format!(
        "Successfully modified '{path_str}' via mpatch ({lines_before} -> {lines_after} lines)."
    ))
}

fn apply_exact_aider_block(original: &str, patch: &str) -> Option<String> {
    const SEARCH_MARKER: &str = "<<<<<<< SEARCH";
    const DIVIDER_MARKER: &str = "=======";
    const REPLACE_MARKER: &str = ">>>>>>> REPLACE";

    let search_idx = patch.find(SEARCH_MARKER)?;
    let divider_idx = patch.find(DIVIDER_MARKER)?;
    let replace_idx = patch.find(REPLACE_MARKER)?;

    if !(search_idx < divider_idx && divider_idx < replace_idx) {
        return None;
    }

    let search_start = search_idx.saturating_add(SEARCH_MARKER.len());
    let search_block = patch.get(search_start..divider_idx)?;
    let replace_start = divider_idx.saturating_add(DIVIDER_MARKER.len());
    let replace_block = patch.get(replace_start..replace_idx)?;

    let search_clean = search_block
        .trim_start_matches(['\r', '\n'])
        .trim_end_matches(['\r', '\n']);
    let replace_clean = replace_block
        .trim_start_matches(['\r', '\n'])
        .trim_end_matches(['\r', '\n']);

    if search_clean.is_empty() {
        return None;
    }

    if original.contains(search_clean) {
        let replaced = original.replacen(search_clean, replace_clean, 1);
        Some(replaced)
    } else {
        None
    }
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
    use serde_json::json;

    use super::*;

    fn create_test_dir(tag: &str) -> std::path::PathBuf {
        let dir = std::env::temp_dir().join(format!(
            "agy-test-mpatch-{tag}-{}",
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        std::fs::create_dir_all(&dir).expect("create test dir");
        dir
    }

    #[test]
    fn test_mpatch_aider_block() {
        let temp = create_test_dir("aider");
        let src_dir = temp.join("src");
        std::fs::create_dir_all(&src_dir).unwrap();
        let file_path = src_dir.join("lib.rs");
        std::fs::write(&file_path, "fn old_func() -> i32 {\n    42\n}\n").unwrap();

        let tool = ToolDefinition {
            name: "modify_code".to_owned(),
            allowed_directories: vec!["src".to_owned()],
            sandbox: agy_common::tools::ToolSandboxPolicy {
                enforce_workspace_bounds: true,
                ..Default::default()
            },
            ..Default::default()
        };

        let patch = "<<<<<<< SEARCH\nfn old_func() -> i32 {\n    42\n}\n=======\nfn new_func() -> i32 {\n    100\n}\n>>>>>>> REPLACE";

        let args = json!({
            "path": "src/lib.rs",
            "patch": patch
        });

        let guards = BTreeMap::new();
        let res = execute(&tool, &temp, &args, None, &guards);
        assert!(res.is_ok());

        let modified = std::fs::read_to_string(&file_path).unwrap();
        assert!(modified.contains("new_func()"));
        assert!(modified.contains("100"));
    }

    #[test]
    fn test_mpatch_mismatched_search_fails() {
        let temp = create_test_dir("mismatch");
        let src_dir = temp.join("src");
        std::fs::create_dir_all(&src_dir).unwrap();
        let file_path = src_dir.join("lib.rs");
        std::fs::write(&file_path, "fn existing_code() {}\n").unwrap();

        let tool = ToolDefinition {
            name: "modify_code".to_owned(),
            allowed_directories: vec!["src".to_owned()],
            ..Default::default()
        };

        let patch = "<<<<<<< SEARCH\nfn non_existent_code() {}\n=======\nfn replacement() {}\n>>>>>>> REPLACE";

        let args = json!({
            "path": "src/lib.rs",
            "patch": patch
        });

        let guards = BTreeMap::new();
        let res = execute(&tool, &temp, &args, None, &guards);
        assert!(res.is_err());
    }

    #[test]
    fn test_mpatch_agent_guard_blocks_unauthorized_dir() {
        let temp = create_test_dir("agent-guard");
        let tests_dir = temp.join("tests");
        std::fs::create_dir_all(&tests_dir).unwrap();
        let file_path = tests_dir.join("test.rs");
        std::fs::write(&file_path, "fn test() {}\n").unwrap();

        let tool = ToolDefinition {
            name: "modify_code".to_owned(),
            sandbox: agy_common::tools::ToolSandboxPolicy {
                enforce_workspace_bounds: true,
                ..Default::default()
            },
            ..Default::default()
        };

        let mut guards = BTreeMap::new();
        guards.insert(
            "worker_agent".to_owned(),
            AgentGuard {
                allowed_dirs: vec!["src".to_owned()],
                ..Default::default()
            },
        );

        let patch = "<<<<<<< SEARCH\nfn test() {}\n=======\nfn test_updated() {}\n>>>>>>> REPLACE";
        let args = json!({
            "path": "tests/test.rs",
            "patch": patch
        });

        assert!(execute(&tool, &temp, &args, Some("worker_agent"), &guards).is_err());
    }

    #[test]
    fn test_mpatch_scoped_path_resolution() {
        let temp = create_test_dir("mpatch-scoped");
        let src_auth = temp.join("src/auth");
        std::fs::create_dir_all(&src_auth).unwrap();
        let file_path = src_auth.join("hasher.rs");
        std::fs::write(&file_path, "fn hash() -> u32 { 10 }\n").unwrap();

        let tool = ToolDefinition {
            name: "modify_code".to_owned(),
            allowed_directories: vec!["src".to_owned()],
            ..Default::default()
        };

        let mut guards = BTreeMap::new();
        guards.insert(
            "worker_agent".to_owned(),
            AgentGuard {
                allowed_dirs: vec!["src".to_owned()],
                ..Default::default()
            },
        );

        let patch = "<<<<<<< SEARCH\nfn hash() -> u32 { 10 }\n=======\nfn hash() -> u32 { 42 }\n>>>>>>> REPLACE";
        // Worker specifies relative scoped path "auth/hasher.rs"
        let args = json!({
            "path": "auth/hasher.rs",
            "patch": patch
        });

        assert!(execute(&tool, &temp, &args, Some("worker_agent"), &guards).is_ok());
        let updated = std::fs::read_to_string(&file_path).unwrap();
        assert!(updated.contains("42"));
    }

    #[test]
    fn test_mpatch_path_policy_enforcement() {
        let temp = create_test_dir("mpatch-policy");
        let src_dir = temp.join("src");
        std::fs::create_dir_all(&src_dir).unwrap();
        let file_path = src_dir.join("lib.rs");
        std::fs::write(&file_path, "pub fn calculate() -> u32 {\n    42\n}\n").unwrap();

        let tool = ToolDefinition {
            name: "modify_code".to_owned(),
            allowed_directories: vec!["src".to_owned()],
            ..Default::default()
        };

        let mut guards = BTreeMap::new();
        guards.insert(
            "tests_agent".to_owned(),
            AgentGuard {
                allowed_dirs: vec!["src".to_owned()],
                path_policies: vec![agy_common::PathPolicy::tests_only("src")],
                ..Default::default()
            },
        );

        // 1. Test agent adds unit test block: allowed
        let valid_patch = "<<<<<<< SEARCH\npub fn calculate() -> u32 {\n    42\n}\n=======\npub fn calculate() -> u32 {\n    42\n}\n\n#[cfg(test)]\nmod tests {\n    #[test]\n    fn t() {}\n}\n>>>>>>> REPLACE";
        let valid_args = json!({
            "path": "src/lib.rs",
            "patch": valid_patch
        });
        assert!(execute(&tool, &temp, &valid_args, Some("tests_agent"), &guards).is_ok());

        // 2. Test agent modifies production code: rejected
        let tampered_patch = "<<<<<<< SEARCH\npub fn calculate() -> u32 {\n    42\n}\n=======\npub fn calculate() -> u32 {\n    100\n}\n>>>>>>> REPLACE";
        let tampered_args = json!({
            "path": "src/lib.rs",
            "patch": tampered_patch
        });
        assert!(execute(&tool, &temp, &tampered_args, Some("tests_agent"), &guards).is_err());
    }
}
