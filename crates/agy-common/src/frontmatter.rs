//! YAML frontmatter parsing and agent manifest extraction.

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
    pub directives: String,
}

impl AgentFrontmatter {
    /// Parses an agent Markdown file content string into an `AgentFrontmatter`.
    ///
    /// # Errors
    /// Returns an error if the opening or closing `---` fences are missing.
    #[allow(
        clippy::too_many_lines,
        clippy::cognitive_complexity,
        reason = "Sequential line-by-line frontmatter parser"
    )]
    pub fn parse(content: &str) -> Result<Self, String> {
        let trimmed = content.trim_start();
        let rest = trimmed.strip_prefix("---").ok_or_else(|| "Missing leading '---'".to_owned())?;
        let end_idx = rest.find("\n---").ok_or_else(|| "Missing closing '---'".to_owned())?;

        let frontmatter = rest.get(..end_idx).unwrap_or("");
        let directives_start = end_idx.saturating_add(4);
        let directives = rest.get(directives_start..).unwrap_or("").trim_start().to_owned();

        let mut name = String::new();
        let mut description = String::new();
        let mut effort = "medium".to_owned();
        let mut permission_mode = "acceptEdits".to_owned();
        let mut command_execution_policy = "off".to_owned();
        let mut main_agent = false;
        let mut subagent = false;
        let mut inherit_mcp = true;
        let mut allowed_subagents = Vec::new();
        let mut pre_launch_checks = Vec::new();
        let mut native_tools = Vec::new();
        let mut mcp_tools = Vec::new();
        let mut allowed_dirs = Vec::new();
        let mut disallowed_dirs = Vec::new();
        let mut allowed_files = Vec::new();
        let mut disallowed_files = Vec::new();

        let mut current_list: Option<&mut Vec<String>> = None;

        for line in frontmatter.lines() {
            let trimmed_line = line.trim();
            if trimmed_line.is_empty() || trimmed_line.starts_with('#') {
                continue;
            }

            // List item handling
            if let Some(item) = trimmed_line.strip_prefix('-') {
                let val = item.trim().trim_matches(['"', '\'']);
                if let Some(ref mut list) = current_list
                    && !val.is_empty()
                    && !val.starts_with("name:")
                    && !val.starts_with("command:")
                {
                    list.push(val.to_owned());
                }
                continue;
            }

            // Reset current list if on a new key
            current_list = None;

            if let Some(val) = trimmed_line.strip_prefix("name:") {
                val.trim().trim_matches(['"', '\'']).clone_into(&mut name);
            } else if let Some(val) = trimmed_line.strip_prefix("description:") {
                val.trim().trim_matches(['"', '\'']).clone_into(&mut description);
            } else if let Some(val) = trimmed_line.strip_prefix("effort:") {
                val.trim().trim_matches(['"', '\'']).clone_into(&mut effort);
            } else if let Some(val) = trimmed_line.strip_prefix("permissionMode:") {
                val.trim().trim_matches(['"', '\'']).clone_into(&mut permission_mode);
            } else if let Some(val) = trimmed_line.strip_prefix("commandExecutionPolicy:") {
                val.trim().trim_matches(['"', '\'']).clone_into(&mut command_execution_policy);
            } else if let Some(val) = trimmed_line.strip_prefix("mainAgent:") {
                main_agent = val.trim().parse::<bool>().unwrap_or(false);
            } else if let Some(val) = trimmed_line.strip_prefix("subagent:") {
                subagent = val.trim().parse::<bool>().unwrap_or(false);
            } else if let Some(val) = trimmed_line.strip_prefix("inheritMcp:") {
                inherit_mcp = val.trim().parse::<bool>().unwrap_or(true);
            } else if trimmed_line.starts_with("allowed_subagents:") {
                current_list = Some(&mut allowed_subagents);
            } else if trimmed_line.starts_with("pre_launch_checks:")
                || trimmed_line.starts_with("preLaunchChecks:")
                || trimmed_line.starts_with("gatekeeper_checks:")
            {
                if trimmed_line.ends_with("[]") {
                    pre_launch_checks.clear();
                } else {
                    current_list = Some(&mut pre_launch_checks);
                }
            } else if trimmed_line.starts_with("tools:") {
                if trimmed_line.ends_with("[]") {
                    native_tools.clear();
                } else {
                    current_list = Some(&mut native_tools);
                }
            } else if trimmed_line.starts_with("mcp_tools:") || trimmed_line.starts_with("mcpTools:") {
                if trimmed_line.ends_with("[]") {
                    mcp_tools.clear();
                } else {
                    current_list = Some(&mut mcp_tools);
                }
            } else if trimmed_line.starts_with("allowed_dirs:") || trimmed_line.starts_with("allowedDirs:") {
                if trimmed_line.ends_with("[]") {
                    allowed_dirs.clear();
                } else {
                    current_list = Some(&mut allowed_dirs);
                }
            } else if trimmed_line.starts_with("disallowed_dirs:") || trimmed_line.starts_with("disallowedDirs:") {
                if trimmed_line.ends_with("[]") {
                    disallowed_dirs.clear();
                } else {
                    current_list = Some(&mut disallowed_dirs);
                }
            } else if trimmed_line.starts_with("allowed_files:") || trimmed_line.starts_with("allowedFiles:") {
                if trimmed_line.ends_with("[]") {
                    allowed_files.clear();
                } else {
                    current_list = Some(&mut allowed_files);
                }
            } else if trimmed_line.starts_with("disallowed_files:") || trimmed_line.starts_with("disallowedFiles:") {
                if trimmed_line.ends_with("[]") {
                    disallowed_files.clear();
                } else {
                    current_list = Some(&mut disallowed_files);
                }
            }
        }

        Ok(Self {
            name,
            description,
            effort,
            permission_mode,
            command_execution_policy,
            main_agent,
            subagent,
            inherit_mcp,
            allowed_subagents,
            pre_launch_checks,
            native_tools,
            mcp_tools,
            allowed_dirs,
            disallowed_dirs,
            allowed_files,
            disallowed_files,
            directives,
        })
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
        {
            None
        } else {
            Some(parsed.to_guard())
        }
    }
}

#[cfg(test)]
#[allow(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::assert_is_empty,
    reason = "Unit test assertions"
)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_agent_frontmatter_full() {
        let md = "---
name: coder
description: Primary implementer
effort: high
permissionMode: default
commandExecutionPolicy: unrestricted
mainAgent: true
subagent: false
inheritMcp: true
allowed_subagents:
  - mini
pre_launch_checks:
  - cargo_check
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
        assert_eq!(manifest.name, "coder");
        assert_eq!(manifest.description, "Primary implementer");
        assert_eq!(manifest.effort, "high");
        assert_eq!(manifest.permission_mode, "default");
        assert_eq!(manifest.command_execution_policy, "unrestricted");
        assert!(manifest.main_agent);
        assert!(!manifest.subagent);
        assert!(manifest.inherit_mcp);
        assert_eq!(manifest.allowed_subagents, vec!["mini"]);
        assert_eq!(manifest.pre_launch_checks, vec!["cargo_check"]);
        assert_eq!(manifest.native_tools, vec!["read_file"]);
        assert_eq!(manifest.mcp_tools, vec!["modify_code", "write_file"]);
        assert_eq!(manifest.directives, "# System Prompt\nDirectives here.\n");
    }

    #[test]
    fn test_parse_agent_frontmatter_guards() {
        let md = "---
name: coder
allowed_dirs:
  - src
disallowed_dirs:
  - tests
  - benches
allowed_files:
  - ARCHITECTURE.md
disallowed_files:
  - Cargo.toml
---
Directives";
        let manifest = AgentFrontmatter::parse(md).unwrap();
        assert_eq!(manifest.allowed_dirs, vec!["src"]);
        assert_eq!(manifest.disallowed_dirs, vec!["tests", "benches"]);
        assert_eq!(manifest.allowed_files, vec!["ARCHITECTURE.md"]);
        assert_eq!(manifest.disallowed_files, vec!["Cargo.toml"]);

        let guard = manifest.to_guard();
        assert_eq!(guard.allowed_dirs, vec!["src"]);
        assert_eq!(guard.disallowed_dirs, vec!["tests", "benches"]);
        assert_eq!(guard.allowed_files, vec!["ARCHITECTURE.md"]);
        assert_eq!(guard.disallowed_files, vec!["Cargo.toml"]);
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
