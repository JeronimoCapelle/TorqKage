//! YAML frontmatter parsing and agent manifest extraction.

use std::{
    fmt::Write,
    fs,
    path::{Path, PathBuf},
};

use serde::{Deserialize, Serialize};

use crate::FastHashSet;

fn default_effort() -> String {
    "medium".to_owned()
}

fn default_permission_mode() -> String {
    "acceptEdits".to_owned()
}

fn default_command_execution_policy() -> String {
    "off".to_owned()
}

const fn default_inherit_mcp() -> bool {
    true
}

/// Parsed metadata and body directives from an agent markdown document.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct AgentFrontmatter {
    #[serde(default)]
    pub name: String,
    #[serde(default)]
    pub description: String,
    #[serde(default = "default_effort")]
    pub effort: String,
    #[serde(
        default = "default_permission_mode",
        rename = "permissionMode",
        alias = "permission_mode"
    )]
    pub permission_mode: String,
    #[serde(
        default = "default_command_execution_policy",
        rename = "commandExecutionPolicy",
        alias = "command_execution_policy"
    )]
    pub command_execution_policy: String,
    #[serde(default, rename = "mainAgent", alias = "main_agent")]
    pub main_agent: bool,
    #[serde(default)]
    pub subagent: bool,
    #[serde(
        default = "default_inherit_mcp",
        rename = "inheritMcp",
        alias = "inherit_mcp"
    )]
    pub inherit_mcp: bool,
    #[serde(default, alias = "allowedSubagents")]
    pub allowed_subagents: Vec<String>,
    #[serde(default, alias = "preLaunchChecks", alias = "gatekeeper_checks")]
    pub pre_launch_checks: Vec<String>,
    #[serde(default, rename = "tools", alias = "native_tools")]
    pub native_tools: Vec<String>,
    #[serde(default, alias = "mcpTools")]
    pub mcp_tools: Vec<String>,
    #[serde(default, alias = "allowedDirs")]
    pub allowed_dirs: Vec<String>,
    #[serde(default, alias = "disallowedDirs")]
    pub disallowed_dirs: Vec<String>,
    #[serde(default, alias = "allowedFiles")]
    pub allowed_files: Vec<String>,
    #[serde(default, alias = "disallowedFiles")]
    pub disallowed_files: Vec<String>,
    #[serde(default, alias = "pathPolicies")]
    pub path_policies: Vec<crate::content_filter::PathPolicy>,
    #[serde(skip)]
    pub directives: String,
}

impl Default for AgentFrontmatter {
    fn default() -> Self {
        Self {
            name: String::new(),
            description: String::new(),
            effort: default_effort(),
            permission_mode: default_permission_mode(),
            command_execution_policy: default_command_execution_policy(),
            main_agent: false,
            subagent: false,
            inherit_mcp: true,
            allowed_subagents: Vec::new(),
            pre_launch_checks: Vec::new(),
            native_tools: Vec::new(),
            mcp_tools: Vec::new(),
            allowed_dirs: Vec::new(),
            disallowed_dirs: Vec::new(),
            allowed_files: Vec::new(),
            disallowed_files: Vec::new(),
            path_policies: Vec::new(),
            directives: String::new(),
        }
    }
}

impl AgentFrontmatter {
    /// Parses an agent Markdown file content string into an `AgentFrontmatter`.
    ///
    /// # Errors
    /// Returns an error if the opening or closing `---` fences are missing or YAML syntax is invalid.
    pub fn parse(content: &str) -> Result<Self, String> {
        let (frontmatter, directives) = parse_fenced_block(content)?;
        let mut manifest: Self = if frontmatter.trim().is_empty() {
            Self::default()
        } else {
            serde_norway::from_str(frontmatter)
                .map_err(|err| format!("Failed to parse agent YAML frontmatter: {err}"))?
        };
        manifest.directives = directives;
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

    /// Serializes this agent manifest to YAML frontmatter + Markdown directives tailored for `harness`.
    #[must_use]
    pub fn serialize_for_harness(&self, harness: &dyn crate::Harness) -> String {
        let mut out = String::new();
        out.push_str("---\n");
        _ = writeln!(out, "name: {}", self.name);
        _ = writeln!(
            out,
            "description: \"{}\"",
            self.description.replace('"', "\\\"")
        );
        _ = writeln!(out, "effort: {}", self.effort);
        if harness.uses_agy_frontmatter() {
            if !self.permission_mode.is_empty() {
                _ = writeln!(out, "permissionMode: {}", self.permission_mode);
            }
            if !self.command_execution_policy.is_empty() {
                _ = writeln!(
                    out,
                    "commandExecutionPolicy: {}",
                    self.command_execution_policy
                );
            }
            if self.main_agent {
                out.push_str("mainAgent: true\n");
            }
            if self.subagent {
                out.push_str("subagent: true\n");
            }
        }
        write_yaml_list(
            &mut out,
            "allowed_subagents",
            &self.allowed_subagents,
            false,
        );
        write_yaml_list(
            &mut out,
            "pre_launch_checks",
            &self.pre_launch_checks,
            false,
        );
        write_yaml_list(&mut out, "allowed_dirs", &self.allowed_dirs, false);
        write_yaml_list(&mut out, "disallowed_dirs", &self.disallowed_dirs, false);
        write_yaml_list(&mut out, "allowed_files", &self.allowed_files, false);
        write_yaml_list(&mut out, "disallowed_files", &self.disallowed_files, false);
        if !self.path_policies.is_empty() {
            out.push_str("path_policies:\n");
            for policy in &self.path_policies {
                _ = writeln!(out, "  - path: {}", policy.path);
                let filter_str = match policy.filter {
                    crate::content_filter::ContentFilterKind::CommentsOnly => "comments_only",
                    crate::content_filter::ContentFilterKind::TestsOnly => "tests_only",
                    crate::content_filter::ContentFilterKind::Delimited => "delimited",
                };
                _ = writeln!(out, "    filter: {filter_str}");
                if let Some(ref start) = policy.start_marker {
                    _ = writeln!(out, "    start_marker: \"{}\"", start.replace('"', "\\\""));
                }
                if let Some(ref end) = policy.end_marker {
                    _ = writeln!(out, "    end_marker: \"{}\"", end.replace('"', "\\\""));
                }
            }
        }
        if harness.uses_agy_frontmatter() {
            if !self.inherit_mcp {
                out.push_str("inheritMcp: false\n");
            }
            out.push_str("mcpServers:\n");
            out.push_str("  - name: tkg-tools\n");
            out.push_str("    command: tkg-mcp\n");
            _ = writeln!(out, "    args: [\"--agent\", \"{}\"]", self.name);
        }

        write_yaml_list(&mut out, "tools", &self.native_tools, true);
        write_yaml_list(&mut out, "mcp_tools", &self.mcp_tools, true);
        out.push_str("---\n\n");
        out.push_str(self.directives.trim_start());
        if !out.ends_with('\n') {
            out.push('\n');
        }
        out
    }

    /// Parses an agent Markdown file from disk, filling in `name` from the file stem if omitted in YAML.
    ///
    /// # Errors
    /// Returns an error if reading the file or parsing YAML frontmatter fails.
    pub fn parse_file(path: &Path) -> Result<Self, String> {
        let content = fs::read_to_string(path)
            .map_err(|e| format!("Failed to read {}: {e}", path.display()))?;
        let mut parsed = Self::parse(&content)?;
        if parsed.name.is_empty()
            && let Some(stem) = path.file_stem().and_then(std::ffi::OsStr::to_str)
        {
            stem.clone_into(&mut parsed.name);
        }
        Ok(parsed)
    }
}

fn write_yaml_list(out: &mut String, key: &str, items: &[String], emit_empty: bool) {
    if items.is_empty() {
        if emit_empty {
            _ = writeln!(out, "{key}: []");
        }
    } else {
        _ = writeln!(out, "{key}:");
        for item in items {
            _ = writeln!(out, "  - {item}");
        }
    }
}

/// Discovers and parses all unique agent manifests across candidate locations for `workspace_root`.
///
/// Returns `(file_path, AgentFrontmatter)` tuples sorted alphabetically by agent name.
#[must_use]
pub fn discover_agent_frontmatters(workspace_root: &Path) -> Vec<(PathBuf, AgentFrontmatter)> {
    let candidate_dirs = candidate_agent_directories(workspace_root);
    let mut agents = Vec::new();
    let mut seen_names = FastHashSet::default();

    for dir in candidate_dirs {
        if !dir.is_dir() {
            continue;
        }
        let Ok(entries) = fs::read_dir(&dir) else {
            continue;
        };

        for entry in entries.flatten() {
            let path = entry.path();
            if path.extension() != Some(std::ffi::OsStr::new("md")) {
                continue;
            }
            let Ok(frontmatter) = AgentFrontmatter::parse_file(&path) else {
                continue;
            };
            if !frontmatter.name.is_empty() && seen_names.insert(frontmatter.name.clone()) {
                agents.push((path, frontmatter));
            }
        }
    }

    agents.sort_by(|a, b| a.1.name.cmp(&b.1.name));
    agents
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

/// Returns the ordered list of candidate directories where agent manifests are located,
/// honoring configured `agent_dirs` and execution harness preferences.
#[must_use]
pub fn candidate_agent_directories_with_config(
    workspace_root: &Path,
    runner: &crate::project::ProjectRunnerConfig,
) -> Vec<PathBuf> {
    let mut dirs = Vec::new();

    // 1. Explicit runner configured directories
    for dir_str in &runner.agent_dirs {
        let p = Path::new(dir_str);
        if p.is_absolute() {
            dirs.push(p.to_path_buf());
        } else {
            dirs.push(workspace_root.join(p));
        }
    }

    // 2. Standard workspace locations
    dirs.push(workspace_root.join("agents"));
    dirs.push(workspace_root.join(".tkg/agents"));
    dirs.push(workspace_root.join("plugins/torqkage/agents"));
    dirs.push(workspace_root.join(".agents/agents"));
    dirs.push(workspace_root.join(".agents"));

    // 3. Harness-specific workspace paths
    let harness = runner.harness();
    dirs.extend(harness.workspace_agent_dirs(workspace_root));

    // 4. User home locations
    if let Ok(home) = std::env::var("HOME") {
        let home_p = PathBuf::from(home);
        dirs.extend(harness.home_agent_dirs(&home_p));
        dirs.push(home_p.join(".agents/agents"));
        dirs.push(home_p.join(".agents"));
    }

    dirs
}

/// Returns the ordered list of candidate directories where agent manifests are located.
#[must_use]
pub fn candidate_agent_directories(workspace_root: &Path) -> Vec<PathBuf> {
    let project_cfg = crate::project::ProjectConfig::detect_or_load(workspace_root);
    let runner = project_cfg.resolved_runner();
    candidate_agent_directories_with_config(workspace_root, &runner)
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

        // Roundtrip serialize_for_harness preserves path_policies
        let serialized = manifest.serialize_for_harness(&crate::harness::Antigravity);
        let roundtripped = AgentFrontmatter::parse(&serialized).unwrap();
        assert_eq!(roundtripped.path_policies, manifest.path_policies);
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
        assert_eq!(checks, Vec::<String>::new());
    }

    #[test]
    fn test_candidate_agent_directories_with_config() {
        let ws = Path::new("/workspace");
        let runner_opencode = crate::project::ProjectRunnerConfig {
            harness: "opencode".to_owned(),
            agent_dirs: vec!["custom_agents".to_owned()],
            ..Default::default()
        };
        let dirs = candidate_agent_directories_with_config(ws, &runner_opencode);
        // Explicit agent_dirs first
        assert_eq!(dirs[0], PathBuf::from("/workspace/custom_agents"));
        // Standard locations
        assert!(dirs.contains(&PathBuf::from("/workspace/agents")));
        // Harness-specific locations
        assert!(dirs.contains(&PathBuf::from("/workspace/.opencode/agents")));
        // Does not include .gemini paths in workspace
        assert!(!dirs.contains(&PathBuf::from("/workspace/.gemini/agents")));
    }
}
