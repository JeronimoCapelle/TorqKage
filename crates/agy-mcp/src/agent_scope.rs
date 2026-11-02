//! Agent scoping and tool allowlisting for `agy-mcp`.

use std::collections::HashSet;
use std::fs;
use std::path::Path;

/// Resolves the requested agent from command line arguments, environment variables, or `.pipeline/active_agent.json`.
#[must_use]
pub(crate) fn resolve_agent_scope(args: &[String], workspace_root: &Path) -> Option<String> {
    // 1. Check CLI args (--agent, -a, --scope, -s)
    let mut i = 1;
    while i < args.len() {
        if let Some(arg) = args.get(i)
            && (arg == "--agent" || arg == "-a" || arg == "--scope" || arg == "-s")
            && let Some(val) = args.get(i.saturating_add(1))
        {
            return Some(val.trim().to_owned());
        }
        i = i.saturating_add(1);
    }

    // 2. Check environment variables (AGY_AGENT, MCP_SCOPE)
    if let Ok(val) = std::env::var("AGY_AGENT")
        && !val.trim().is_empty()
    {
        return Some(val.trim().to_owned());
    }
    if let Ok(val) = std::env::var("MCP_SCOPE")
        && !val.trim().is_empty()
    {
        return Some(val.trim().to_owned());
    }

    // 3. Check persistent pipeline state bridge (.pipeline/active_agent.json or .pipeline/active_agent)
    if let Some(agent) = read_active_agent_from_pipeline(workspace_root) {
        return Some(agent);
    }

    None
}

/// Reads the active agent identity from `.pipeline/active_agent.json` or `.pipeline/active_agent`.
fn read_active_agent_from_pipeline(workspace_root: &Path) -> Option<String> {
    let json_path = workspace_root.join(".pipeline/active_agent.json");
    if json_path.is_file()
        && let Ok(content) = fs::read_to_string(&json_path)
        && let Ok(val) = serde_json::from_str::<serde_json::Value>(&content)
        && let Some(name) = val.get("agent").and_then(serde_json::Value::as_str)
        && !name.trim().is_empty()
    {
        return Some(name.trim().to_owned());
    }

    let txt_path = workspace_root.join(".pipeline/active_agent");
    if txt_path.is_file()
        && let Ok(content) = fs::read_to_string(&txt_path)
    {
        let name = content.trim();
        if !name.is_empty() {
            return Some(name.to_owned());
        }
    }

    None
}

/// Resolves the list of allowed MCP tools for the given agent name.
/// Reads directly from the agent Markdown frontmatter (`mcp_tools:`).
/// If no agent file exists or if no tools are listed, returns an empty set (air-gapped default deny).
#[must_use]
pub(crate) fn resolve_allowed_tools(workspace_root: &Path, agent_name: &str) -> HashSet<String> {
    parse_tools_from_agent_files(workspace_root, agent_name).unwrap_or_default()
}

fn parse_tools_from_agent_files(workspace_root: &Path, agent_name: &str) -> Option<HashSet<String>> {
    let path = agy_common::find_agent_manifest_path(workspace_root, agent_name)?;
    if path.is_file()
        && let Ok(content) = fs::read_to_string(&path)
        && let Some(tools) = extract_mcp_tools_from_markdown(&content)
    {
        return Some(tools);
    }

    None
}

/// Extracts tools listed under `mcp_tools:` or `mcpTools:` in the YAML frontmatter.
#[must_use]
pub(crate) fn extract_mcp_tools_from_markdown(content: &str) -> Option<HashSet<String>> {
    agy_common::frontmatter::AgentFrontmatter::extract_mcp_tools(content)
}

#[cfg(test)]
#[allow(clippy::unwrap_used, clippy::expect_used, reason = "Unit tests allow panics on test setup")]
mod tests {
    use super::*;

    #[test]
    fn test_extract_mcp_tools_from_markdown() {
        let md = "---\nname: coder\ntools: []\nmcp_tools:\n  - read_project_file\n  - modify_code\n  - write_file\n---\nSome body\n";
        let tools = extract_mcp_tools_from_markdown(md).unwrap();
        assert_eq!(tools.len(), 3);
        assert!(tools.contains("read_project_file"));
        assert!(tools.contains("modify_code"));
        assert!(tools.contains("write_file"));
    }

    #[test]
    fn test_resolve_allowed_tools_unconfigured_agent_is_airgapped() {
        let tools = resolve_allowed_tools(Path::new("."), "nonexistent-agent-without-file");
        assert!(tools.is_empty());
    }

    #[test]
    fn test_read_active_agent_from_pipeline() {
        let temp_dir = std::env::temp_dir().join(format!("agy_agent_scope_test_{}", std::process::id()));
        let pipeline_dir = temp_dir.join(".pipeline");
        fs::create_dir_all(&pipeline_dir).expect("create pipeline dir");

        // 1. JSON state
        fs::write(pipeline_dir.join("active_agent.json"), r#"{"agent": "test-author"}"#).expect("write json");
        assert_eq!(read_active_agent_from_pipeline(&temp_dir), Some("test-author".to_owned()));

        // 2. Text fallback
        fs::remove_file(pipeline_dir.join("active_agent.json")).expect("remove json");
        fs::write(pipeline_dir.join("active_agent"), "finisher\n").expect("write txt");
        assert_eq!(read_active_agent_from_pipeline(&temp_dir), Some("finisher".to_owned()));

        drop(fs::remove_dir_all(&temp_dir));
    }
}
