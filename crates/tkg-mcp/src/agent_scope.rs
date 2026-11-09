//! Agent scoping and tool allowlisting for `tkg-mcp`.

use std::{collections::BTreeMap, fs, path::Path};

use tkg_common::{FastHashSet, frontmatter::AgentFrontmatter};

use crate::models::AgentGuard;

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

    // 2. Check environment variables and persistent agent state bridge via tkg-common
    tkg_common::resolve_active_agent(workspace_root)
}

/// Resolves the list of allowed MCP tools for the given agent name.
/// Reads directly from the agent Markdown frontmatter (`mcp_tools:`).
/// If no agent file exists or if no tools are listed, returns an empty set (air-gapped default deny).
#[must_use]
pub(crate) fn resolve_allowed_tools(
    workspace_root: &Path,
    agent_name: &str,
) -> FastHashSet<String> {
    parse_tools_from_agent_files(workspace_root, agent_name).unwrap_or_default()
}

fn parse_tools_from_agent_files(
    workspace_root: &Path,
    agent_name: &str,
) -> Option<FastHashSet<String>> {
    let path = tkg_common::find_agent_manifest_path(workspace_root, agent_name)?;
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
pub(crate) fn extract_mcp_tools_from_markdown(content: &str) -> Option<FastHashSet<String>> {
    AgentFrontmatter::extract_mcp_tools(content)
}

/// Discovers and parses `AgentGuard` configurations directly from agent Markdown frontmatters.
#[must_use]
pub(crate) fn resolve_agent_guards(
    workspace_root: &Path,
    active_agent: Option<&str>,
) -> BTreeMap<String, AgentGuard> {
    let mut guards: BTreeMap<String, AgentGuard> =
        tkg_common::discover_agent_frontmatters(workspace_root)
            .into_iter()
            .map(|(_path, fm)| {
                let guard = fm.to_guard();
                (fm.name, guard)
            })
            .collect();

    // Ensure active agent is loaded if not already found in directory scans
    if let Some(agent) = active_agent
        && !guards.contains_key(agent)
        && let Some(path) = tkg_common::find_agent_manifest_path(workspace_root, agent)
        && let Ok(frontmatter) = AgentFrontmatter::parse_file(&path)
    {
        guards.insert(agent.to_owned(), frontmatter.to_guard());
    }

    guards
}

#[cfg(test)]
#[allow(
    clippy::unwrap_used,
    clippy::expect_used,
    reason = "Unit tests allow panics on test setup"
)]
mod tests {
    use super::*;

    #[test]
    fn test_extract_mcp_tools_from_markdown() {
        let md = "---\nname: mock_agent\ntools: []\nmcp_tools:\n  - read_project_file\n  - modify_code\n  - write_file\n---\nSome body\n";
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
    fn test_read_active_agent_from_state() {
        let temp_dir =
            std::env::temp_dir().join(format!("tkg_agent_scope_test_{}", std::process::id()));
        let paths = tkg_common::TkgStatePaths::new(&temp_dir);
        let state_dir = paths.state_dir();
        fs::create_dir_all(&state_dir).expect("create state dir");

        // 1. JSON state
        fs::write(paths.active_agent_file(), r#"{"agent": "agent_alpha"}"#).expect("write json");
        assert_eq!(
            tkg_common::resolve_active_agent(&temp_dir),
            Some("agent_alpha".to_owned())
        );

        // 2. Text fallback
        fs::remove_file(paths.active_agent_file()).expect("remove json");
        fs::write(state_dir.join("active_agent"), "agent_beta\n").expect("write txt");
        assert_eq!(
            tkg_common::resolve_active_agent(&temp_dir),
            Some("agent_beta".to_owned())
        );

        drop(fs::remove_dir_all(&temp_dir));
    }
}
