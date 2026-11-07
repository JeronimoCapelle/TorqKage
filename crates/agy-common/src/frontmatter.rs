//! YAML frontmatter parsing and agent manifest extraction.

use std::path::{Path, PathBuf};

use crate::FastHashSet;

/// Parsed metadata and body directives from an agent markdown document.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct AgentFrontmatter {
    pub name: String,
    pub description: String,
    pub effort: String,
    pub permission_mode: String,
    pub command_execution_policy: String,
    pub main_agent: bool,
    pub subagent: bool,
    pub inherit_mcp: bool,
    pub allowed_subagents: Vec<String>,
    pub pre_launch_checks: Vec<String>,
    pub native_tools: Vec<String>,
    pub mcp_tools: Vec<String>,
    pub allowed_dirs: Vec<String>,
    pub disallowed_dirs: Vec<String>,
    pub allowed_files: Vec<String>,
    pub disallowed_files: Vec<String>,
    pub path_policies: Vec<crate::content_filter::PathPolicy>,
    pub directives: String,
}

impl AgentFrontmatter {
    /// Parses an agent Markdown file content string into an `AgentFrontmatter`.
    ///
    /// # Errors
    /// Returns an error if the opening or closing `---` fences are missing.
    pub fn parse(content: &str) -> Result<Self, String> {
        let (frontmatter, directives) = parse_fenced_block(content)?;
        let mut manifest = Self {
            effort: "medium".to_owned(),
            permission_mode: "acceptEdits".to_owned(),
            command_execution_policy: "off".to_owned(),
            inherit_mcp: true,
            directives,
            ..Default::default()
        };

        let mut current_list = None;
        let mut in_path_policies = false;

        for line in frontmatter.lines() {
            let trimmed_line = line.trim();
            if trimmed_line.is_empty() || trimmed_line.starts_with('#') {
                continue;
            }

            // Top-level key detection resets active sub-sections
            let is_top_level =
                !line.starts_with(char::is_whitespace) && !trimmed_line.starts_with('-');
            if is_top_level {
                in_path_policies = false;
                current_list = None;
            }

            if in_path_policies {
                if let Some(item) = trimmed_line.strip_prefix('-') {
                    if let Some(pol) = parse_path_policy_inline(item) {
                        manifest.path_policies.push(pol);
                    }
                    continue;
                }
                if let Some(last) = manifest.path_policies.last_mut()
                    && apply_path_policy_subfield(last, trimmed_line)
                {
                    continue;
                }
            }

            // List item handling
            if let Some(item) = trimmed_line.strip_prefix('-') {
                let val = item.trim().trim_matches(['"', '\'']);
                if let Some(active) = current_list
                    && !val.is_empty()
                    && !val.starts_with("name:")
                    && !val.starts_with("command:")
                {
                    append_to_active_list(&mut manifest, active, val);
                }
                continue;
            }

            // Reset current list if on a new key
            current_list = None;
            if apply_scalar_field(&mut manifest, trimmed_line) {
                continue;
            }
            if let Some(active) =
                select_active_list(&mut manifest, trimmed_line, &mut in_path_policies)
            {
                current_list = Some(active);
            }
        }

        Ok(manifest)
    }

    /// Extracts the set of allowed MCP tools from the agent markdown.
    #[must_use]
    pub fn extract_mcp_tools(content: &str) -> Option<FastHashSet<String>> {
        let parsed = Self::parse(content).ok()?;
        if parsed.mcp_tools.is_empty() {
            None
        } else {
            Some(parsed.mcp_tools.into_iter().collect())
        }
    }

    /// Extracts `pre_launch_checks` from the agent markdown.
    #[must_use]
    pub fn extract_pre_launch_checks(content: &str) -> Vec<String> {
        Self::parse(content).map_or_default(|f| f.pre_launch_checks)
    }

    /// Converts the parsed frontmatter into an `AgentGuard`.
    #[must_use]
    pub fn to_guard(&self) -> crate::models::AgentGuard {
        crate::models::AgentGuard {
            allowed_dirs: self.allowed_dirs.clone(),
            disallowed_dirs: self.disallowed_dirs.clone(),
            allowed_files: self.allowed_files.clone(),
            disallowed_files: self.disallowed_files.clone(),
            path_policies: self.path_policies.clone(),
        }
    }

    /// Extracts an `AgentGuard` from an agent Markdown document if any boundaries are defined.
    #[must_use]
    pub fn extract_guard(content: &str) -> Option<crate::models::AgentGuard> {
        let parsed = Self::parse(content).ok()?;
        if parsed.allowed_dirs.is_empty()
            && parsed.disallowed_dirs.is_empty()
            && parsed.allowed_files.is_empty()
            && parsed.disallowed_files.is_empty()
            && parsed.path_policies.is_empty()
        {
            None
        } else {
            Some(parsed.to_guard())
        }
    }
}

fn parse_inline_yaml_list(s: &str) -> Vec<String> {
    let trimmed = s.trim().trim_matches(['[', ']']);
    trimmed
        .split(',')
        .map(|item| item.trim().trim_matches(['"', '\'']).to_owned())
        .filter(|item| !item.is_empty())
        .collect()
}

fn parse_fenced_block(content: &str) -> Result<(&str, String), String> {
    let trimmed = content.trim_start();
    let rest = trimmed
        .strip_prefix("---")
        .ok_or_else(|| "Missing leading '---'".to_owned())?;
    let end_idx = rest
        .find("\n---")
        .ok_or_else(|| "Missing closing '---'".to_owned())?;

    let frontmatter = rest.get(..end_idx).unwrap_or("");
    let directives_start = end_idx.saturating_add(4);
    let directives = rest
        .get(directives_start..)
        .unwrap_or("")
        .trim_start()
        .to_owned();
    Ok((frontmatter, directives))
}

fn parse_filter_kind(val: &str) -> crate::content_filter::ContentFilterKind {
    match val {
        "tests_only" | "tests" => crate::content_filter::ContentFilterKind::TestsOnly,
        "delimited" => crate::content_filter::ContentFilterKind::Delimited,
        _ => crate::content_filter::ContentFilterKind::CommentsOnly,
    }
}

fn parse_path_policy_inline(item: &str) -> Option<crate::content_filter::PathPolicy> {
    let item_trim = item.trim();
    if item_trim.starts_with('{')
        && item_trim.ends_with('}')
        && let Ok(pol) = serde_json::from_str::<crate::content_filter::PathPolicy>(item_trim)
    {
        return Some(pol);
    }

    let mut path = String::new();
    let mut filter = crate::content_filter::ContentFilterKind::CommentsOnly;
    let mut start_marker = None;
    let mut end_marker = None;

    for part in item_trim.split(',') {
        let part = part.trim();
        if let Some(p) = part.strip_prefix("path:") {
            p.trim().trim_matches(['"', '\'']).clone_into(&mut path);
        } else if let Some(f) = part.strip_prefix("filter:") {
            filter = parse_filter_kind(f.trim().trim_matches(['"', '\'']));
        } else if let Some(m) = part.strip_prefix("start_marker:") {
            start_marker = Some(m.trim().trim_matches(['"', '\'']).to_owned());
        } else if let Some(m) = part.strip_prefix("end_marker:") {
            end_marker = Some(m.trim().trim_matches(['"', '\'']).to_owned());
        }
    }

    if path.is_empty() {
        None
    } else {
        Some(crate::content_filter::PathPolicy {
            path,
            filter,
            start_marker,
            end_marker,
            ..Default::default()
        })
    }
}

fn apply_path_policy_subfield(
    policy: &mut crate::content_filter::PathPolicy,
    trimmed_line: &str,
) -> bool {
    if let Some(f) = trimmed_line.strip_prefix("filter:") {
        policy.filter = parse_filter_kind(f.trim().trim_matches(['"', '\'']));
        true
    } else if let Some(p) = trimmed_line.strip_prefix("path:") {
        p.trim()
            .trim_matches(['"', '\''])
            .clone_into(&mut policy.path);
        true
    } else if let Some(m) = trimmed_line.strip_prefix("start_marker:") {
        policy.start_marker = Some(m.trim().trim_matches(['"', '\'']).to_owned());
        true
    } else if let Some(m) = trimmed_line.strip_prefix("end_marker:") {
        policy.end_marker = Some(m.trim().trim_matches(['"', '\'']).to_owned());
        true
    } else if let Some(items) = trimmed_line.strip_prefix("line_comments:") {
        policy.line_comments = parse_inline_yaml_list(items);
        true
    } else if let Some(items) = trimmed_line.strip_prefix("test_attributes:") {
        policy.test_attributes = parse_inline_yaml_list(items);
        true
    } else if let Some(items) = trimmed_line.strip_prefix("test_prefixes:") {
        policy.test_prefixes = parse_inline_yaml_list(items);
        true
    } else {
        false
    }
}

fn apply_scalar_field(manifest: &mut AgentFrontmatter, trimmed_line: &str) -> bool {
    if let Some(val) = trimmed_line.strip_prefix("name:") {
        val.trim()
            .trim_matches(['"', '\''])
            .clone_into(&mut manifest.name);
        true
    } else if let Some(val) = trimmed_line.strip_prefix("description:") {
        val.trim()
            .trim_matches(['"', '\''])
            .clone_into(&mut manifest.description);
        true
    } else if let Some(val) = trimmed_line.strip_prefix("effort:") {
        val.trim()
            .trim_matches(['"', '\''])
            .clone_into(&mut manifest.effort);
        true
    } else if let Some(val) = trimmed_line.strip_prefix("permissionMode:") {
        val.trim()
            .trim_matches(['"', '\''])
            .clone_into(&mut manifest.permission_mode);
        true
    } else if let Some(val) = trimmed_line.strip_prefix("commandExecutionPolicy:") {
        val.trim()
            .trim_matches(['"', '\''])
            .clone_into(&mut manifest.command_execution_policy);
        true
    } else if let Some(val) = trimmed_line.strip_prefix("mainAgent:") {
        manifest.main_agent = val.trim().parse::<bool>().unwrap_or(false);
        true
    } else if let Some(val) = trimmed_line.strip_prefix("subagent:") {
        manifest.subagent = val.trim().parse::<bool>().unwrap_or(false);
        true
    } else if let Some(val) = trimmed_line.strip_prefix("inheritMcp:") {
        manifest.inherit_mcp = val.trim().parse::<bool>().unwrap_or(true);
        true
    } else {
        false
    }
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum ActiveListKind {
    AllowedSubagents,
    PreLaunchChecks,
    NativeTools,
    McpTools,
    AllowedDirs,
    DisallowedDirs,
    AllowedFiles,
    DisallowedFiles,
}

fn select_active_list(
    manifest: &mut AgentFrontmatter,
    trimmed_line: &str,
    in_path_policies: &mut bool,
) -> Option<ActiveListKind> {
    if trimmed_line.starts_with("allowed_subagents:") {
        Some(ActiveListKind::AllowedSubagents)
    } else if trimmed_line.starts_with("pre_launch_checks:")
        || trimmed_line.starts_with("preLaunchChecks:")
        || trimmed_line.starts_with("gatekeeper_checks:")
    {
        if trimmed_line.ends_with("[]") {
            manifest.pre_launch_checks.clear();
            None
        } else {
            Some(ActiveListKind::PreLaunchChecks)
        }
    } else if trimmed_line.starts_with("tools:") {
        if trimmed_line.ends_with("[]") {
            manifest.native_tools.clear();
            None
        } else {
            Some(ActiveListKind::NativeTools)
        }
    } else if trimmed_line.starts_with("mcp_tools:") || trimmed_line.starts_with("mcpTools:") {
        if trimmed_line.ends_with("[]") {
            manifest.mcp_tools.clear();
            None
        } else {
            Some(ActiveListKind::McpTools)
        }
    } else if trimmed_line.starts_with("allowed_dirs:") || trimmed_line.starts_with("allowedDirs:")
    {
        if trimmed_line.ends_with("[]") {
            manifest.allowed_dirs.clear();
            None
        } else {
            Some(ActiveListKind::AllowedDirs)
        }
    } else if trimmed_line.starts_with("disallowed_dirs:")
        || trimmed_line.starts_with("disallowedDirs:")
    {
        if trimmed_line.ends_with("[]") {
            manifest.disallowed_dirs.clear();
            None
        } else {
            Some(ActiveListKind::DisallowedDirs)
        }
    } else if trimmed_line.starts_with("allowed_files:")
        || trimmed_line.starts_with("allowedFiles:")
    {
        if trimmed_line.ends_with("[]") {
            manifest.allowed_files.clear();
            None
        } else {
            Some(ActiveListKind::AllowedFiles)
        }
    } else if trimmed_line.starts_with("disallowed_files:")
        || trimmed_line.starts_with("disallowedFiles:")
    {
        if trimmed_line.ends_with("[]") {
            manifest.disallowed_files.clear();
            None
        } else {
            Some(ActiveListKind::DisallowedFiles)
        }
    } else if trimmed_line.starts_with("path_policies:")
        || trimmed_line.starts_with("pathPolicies:")
    {
        if trimmed_line.ends_with("[]") {
            manifest.path_policies.clear();
            *in_path_policies = false;
        } else {
            *in_path_policies = true;
        }
        None
    } else {
        None
    }
}

fn append_to_active_list(manifest: &mut AgentFrontmatter, list_kind: ActiveListKind, item: &str) {
    let target = match list_kind {
        ActiveListKind::AllowedSubagents => &mut manifest.allowed_subagents,
        ActiveListKind::PreLaunchChecks => &mut manifest.pre_launch_checks,
        ActiveListKind::NativeTools => &mut manifest.native_tools,
        ActiveListKind::McpTools => &mut manifest.mcp_tools,
        ActiveListKind::AllowedDirs => &mut manifest.allowed_dirs,
        ActiveListKind::DisallowedDirs => &mut manifest.disallowed_dirs,
        ActiveListKind::AllowedFiles => &mut manifest.allowed_files,
        ActiveListKind::DisallowedFiles => &mut manifest.disallowed_files,
    };
    target.push(item.to_owned());
}

/// Returns the ordered list of candidate directories where agent manifests are located.
///
/// Precedence:
/// 1. `<workspace>/agents`
/// 2. `<workspace>/plugins/agy-suite/agents`
/// 3. `<workspace>/.agents/agents`
/// 4. `<workspace>/.agents`
/// 5. `<workspace>/.gemini/agents`
/// 6. `<workspace>/.agy/agents`
/// 7. `~/.gemini/config/plugins/agy-suite/agents`
/// 8. `~/.gemini/agents`
/// 9. `~/.agents/agents`
/// 10. `~/.agents`
#[must_use]
pub fn candidate_agent_directories(workspace_root: &Path) -> Vec<PathBuf> {
    let mut dirs = vec![
        workspace_root.join("agents"),
        workspace_root.join("plugins/agy-suite/agents"),
        workspace_root.join(".agents/agents"),
        workspace_root.join(".agents"),
        workspace_root.join(".gemini/agents"),
        workspace_root.join(".agy/agents"),
    ];

    if let Ok(home) = std::env::var("HOME") {
        let home_p = PathBuf::from(home);
        dirs.push(home_p.join(".gemini/config/plugins/agy-suite/agents"));
        dirs.push(home_p.join(".gemini/agents"));
        dirs.push(home_p.join(".agents/agents"));
        dirs.push(home_p.join(".agents"));
    }

    dirs
}

/// Discovers an agent markdown file across candidate locations by its name.
#[must_use]
pub fn find_agent_manifest(workspace_root: &Path, agent_name: &str) -> Option<PathBuf> {
    let file_name = if Path::new(agent_name)
        .extension()
        .is_some_and(|ext| ext.eq_ignore_ascii_case("md"))
    {
        agent_name.to_owned()
    } else {
        format!("{agent_name}.md")
    };

    for dir in candidate_agent_directories(workspace_root) {
        let candidate = dir.join(&file_name);
        if candidate.is_file() {
            return Some(candidate);
        }
    }
    None
}

#[cfg(test)]
#[allow(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::assert_is_empty,
    clippy::indexing_slicing,
    reason = "Unit test assertions"
)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_agent_frontmatter_full() {
        let md = "---
name: agent_primary
description: Primary implementer
effort: high
permissionMode: default
commandExecutionPolicy: unrestricted
mainAgent: true
subagent: false
inheritMcp: true
allowed_subagents:
  - subagent_helper
pre_launch_checks:
  - check_gate
tools:
  - read_file
mcp_tools:
  - modify_code
  - write_file
---
# System Prompt
Directives here.
";
        let manifest = AgentFrontmatter::parse(md).unwrap();
        assert_eq!(manifest.name, "agent_primary");
        assert_eq!(manifest.description, "Primary implementer");
        assert_eq!(manifest.effort, "high");
        assert_eq!(manifest.permission_mode, "default");
        assert_eq!(manifest.command_execution_policy, "unrestricted");
        assert!(manifest.main_agent);
        assert!(!manifest.subagent);
        assert!(manifest.inherit_mcp);
        assert_eq!(manifest.allowed_subagents, vec!["subagent_helper"]);
        assert_eq!(manifest.pre_launch_checks, vec!["check_gate"]);
        assert_eq!(manifest.native_tools, vec!["read_file"]);
        assert_eq!(manifest.mcp_tools, vec!["modify_code", "write_file"]);
        assert_eq!(manifest.directives, "# System Prompt\nDirectives here.\n");
    }

    #[test]
    fn test_parse_agent_frontmatter_path_policies() {
        let md = "---
name: agent_restricted
description: Technical writer
allowed_dirs:
  - docs
  - src
path_policies:
  - path: src
    filter: comments_only
  - path: src/autogen.rs
    filter: delimited
    start_marker: \"// START\"
    end_marker: \"// END\"
---
# Directives
";
        let manifest = AgentFrontmatter::parse(md).unwrap();
        assert_eq!(manifest.name, "agent_restricted");
        assert_eq!(manifest.allowed_dirs, vec!["docs", "src"]);
        assert_eq!(manifest.path_policies.len(), 2);
        assert_eq!(manifest.path_policies[0].path, "src");
        assert_eq!(
            manifest.path_policies[0].filter,
            crate::content_filter::ContentFilterKind::CommentsOnly
        );
        assert_eq!(manifest.path_policies[1].path, "src/autogen.rs");
        assert_eq!(
            manifest.path_policies[1].filter,
            crate::content_filter::ContentFilterKind::Delimited
        );
        assert_eq!(
            manifest.path_policies[1].start_marker.as_deref(),
            Some("// START")
        );
        assert_eq!(
            manifest.path_policies[1].end_marker.as_deref(),
            Some("// END")
        );

        let guard = manifest.to_guard();
        assert_eq!(guard.path_policies.len(), 2);
    }

    #[test]
    fn test_parse_agent_frontmatter_guards() {
        let md = "---
name: agent_scoped
allowed_dirs:
  - src
disallowed_dirs:
  - tests
  - benches
allowed_files:
  - spec.md
disallowed_files:
  - protected.env
---
Directives";
        let manifest = AgentFrontmatter::parse(md).unwrap();
        assert_eq!(manifest.allowed_dirs, vec!["src"]);
        assert_eq!(manifest.disallowed_dirs, vec!["tests", "benches"]);
        assert_eq!(manifest.allowed_files, vec!["spec.md"]);
        assert_eq!(manifest.disallowed_files, vec!["protected.env"]);

        let guard = manifest.to_guard();
        assert_eq!(guard.allowed_dirs, vec!["src"]);
        assert_eq!(guard.disallowed_dirs, vec!["tests", "benches"]);
        assert_eq!(guard.allowed_files, vec!["spec.md"]);
        assert_eq!(guard.disallowed_files, vec!["protected.env"]);
    }

    #[test]
    fn test_extract_mcp_tools() {
        let md = "---\nname: test\nmcp_tools:\n  - a\n  - b\n---\nbody";
        let tools = AgentFrontmatter::extract_mcp_tools(md).unwrap();
        assert_eq!(tools.len(), 2);
        assert!(tools.contains("a"));
        assert!(tools.contains("b"));
    }

    #[test]
    fn test_extract_pre_launch_checks_empty_list() {
        let md = "---\nname: test\npre_launch_checks: []\n---\nbody";
        let checks = AgentFrontmatter::extract_pre_launch_checks(md);
        assert!(checks.is_empty());
    }
}
