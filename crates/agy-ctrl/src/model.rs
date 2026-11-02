//! Data models, YAML frontmatter parser/serializer, and tool catalog definitions.

use std::collections::{BTreeMap, HashSet};
use std::fs;
use std::path::{Path, PathBuf};
use serde::Deserialize;

/// Category of an MCP tool for filtering and visual grouping.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub(crate) enum ToolCategory {
    /// Compilation & code verification (`cargo check`, `cargo clippy`, `rustfmt`)
    Compilation,
    /// Automated testing, coverage & fuzzing (`nextest`, `llvm-cov`, `mutants`, `fuzz`)
    Testing,
    /// Hardware performance, microarchitecture, benchmarks & profiling (`asm`, `llvm-mca`, `samply`, `divan`)
    Profiling,
    /// Transactional, bounded filesystem operations (`read_project_file`, `mpatch`)
    Filesystem,
    /// Supply-chain, licensing, `SemVer` & changelog auditing (`cargo-deny`, `git-cliff`)
    Release,
    /// Repository state, Git/JJ inspection, specs & manuals (`diff`, `conflicts`, `tldr`)
    Diagnostics,
    /// Uncategorized / custom
    Other,
}

impl ToolCategory {
    /// Returns the human-readable display label.
    #[must_use]
    pub(crate) const fn label(self) -> &'static str {
        match self {
            Self::Compilation => "Compilation & Lints",
            Self::Testing => "Testing & Verification",
            Self::Profiling => "Hardware & Profiling",
            Self::Filesystem => "Filesystem & Edit",
            Self::Release => "Release & SemVer",
            Self::Diagnostics => "State & Diagnostics",
            Self::Other => "General",
        }
    }
}

/// In-memory representation of an MCP tool loaded from `tools.toml`.
#[derive(Debug, Clone)]
pub(crate) struct ToolItem {
    pub(crate) name: String,
    pub(crate) description: String,
    pub(crate) action: String,
    pub(crate) category: ToolCategory,
    pub(crate) command: Option<Vec<String>>,
    pub(crate) allowed_directories: Option<Vec<String>>,
    pub(crate) timeout_seconds: Option<u64>,
    pub(crate) output_capture: Option<String>,
    pub(crate) ban_dangerous_files: bool,
    pub(crate) enforce_workspace_bounds: bool,
    pub(crate) attach_rustc_explain: bool,
    pub(crate) require_clippy_clean: bool,
    pub(crate) require_tests_passed: bool,
    pub(crate) require_git_clean: bool,
    pub(crate) require_permission_gate: bool,
}

impl ToolItem {
    /// Infers the logical category based on the tool name and action.
    #[must_use]
    pub(crate) fn infer_category(name: &str) -> ToolCategory {
        if name.starts_with("run_cargo_clippy")
            || name.starts_with("run_cargo_verification")
            || name == "run_quick_check"
            || name == "run_cargo_fmt"
            || name == "run_cargo_check_targets"
            || name == "run_cargo_expand"
            || name == "run_cargo_zigbuild"
        {
            ToolCategory::Compilation
        } else if name.starts_with("run_cargo_test")
            || name.starts_with("run_cargo_nextest")
            || name.starts_with("run_coverage")
            || name.starts_with("run_mutation")
            || name.starts_with("run_cargo_insta")
            || name.starts_with("run_cargo_fuzz")
            || name.starts_with("run_property")
            || name.starts_with("run_concurrency")
            || name == "run_qemu_kernel_test"
        {
            ToolCategory::Testing
        } else if name.starts_with("run_cargo_bench")
            || name.starts_with("run_gungraun")
            || name.starts_with("run_criterion")
            || name.starts_with("run_divan")
            || name.starts_with("run_cargo_asm")
            || name.starts_with("run_flamegraph")
            || name.starts_with("run_samply")
            || name.starts_with("run_cargo_perf")
            || name.starts_with("run_cargo_bloat")
            || name.starts_with("run_hyperfine")
            || name.starts_with("run_valgrind")
            || name.starts_with("run_strace")
            || name.starts_with("analyze_llvm")
            || name.starts_with("inspect_binary")
            || name.starts_with("inspect_symbol")
            || name == "run_cargo_careful"
            || name == "run_cargo_miri"
        {
            ToolCategory::Profiling
        } else if name.contains("project_file")
            || name.contains("source_code")
            || name.contains("source_file")
            || name.contains("test_code")
            || name.contains("test_file")
            || name.contains("design_doc")
            || name == "search_codebase"
        {
            ToolCategory::Filesystem
        } else if name.starts_with("run_cargo_deny")
            || name.starts_with("run_cargo_machete")
            || name.starts_with("run_cargo_semver")
            || name.starts_with("run_cargo_public_api")
            || name.starts_with("run_cargo_hack")
            || name.starts_with("run_cargo_outdated")
            || name.starts_with("run_cargo_doc")
            || name.starts_with("run_git_cliff")
        {
            ToolCategory::Release
        } else if name.starts_with("inspect_")
            || name.starts_with("query_")
            || name == "eval_symbolic_math"
            || name == "run_ast_grep"
            || name == "run_nasm_check"
        {
            ToolCategory::Diagnostics
        } else {
            ToolCategory::Other
        }
    }
}

/// Raw representation used for parsing `tools.toml`.
#[derive(Debug, Clone, Deserialize, Default)]
struct RawToolToml {
    inherit: Option<String>,
    description: Option<String>,
    action: Option<String>,
    command: Option<Vec<String>>,
    allowed_directories: Option<Vec<String>>,
    timeout_seconds: Option<u64>,
    output_capture: Option<String>,
    ban_dangerous_files: Option<bool>,
    enforce_workspace_bounds: Option<bool>,
    attach_rustc_explain: Option<bool>,
    require_clippy_clean: Option<bool>,
    require_tests_passed: Option<bool>,
    require_git_clean: Option<bool>,
    require_permission_gate: Option<bool>,
}

#[derive(Debug, Clone, Deserialize, Default)]
struct RawToolsFile {
    #[serde(default)]
    base_tools: BTreeMap<String, RawToolToml>,
    #[serde(default)]
    tools: BTreeMap<String, RawToolToml>,
}

/// Loads all tools from a `tools.toml` file path.
pub(crate) fn load_tools_from_file(path: &Path) -> Result<Vec<ToolItem>, String> {
    let content = fs::read_to_string(path).map_err(|e| format!("Failed to read {}: {e}", path.display()))?;
    let raw: RawToolsFile = toml::from_str(&content).map_err(|e| format!("Failed to parse {}: {e}", path.display()))?;

    let mut items = Vec::with_capacity(raw.tools.len());

    for (name, raw_tool) in raw.tools {
        let base = raw_tool.inherit.as_deref().and_then(|key| {
            let clean = key.strip_prefix("base_tools.").unwrap_or(key);
            raw.base_tools.get(clean)
        });

        let description = raw_tool
            .description
            .or_else(|| {
                let b = base?;
                b.description.clone()
            })
            .unwrap_or_default();
        let action = raw_tool
            .action
            .or_else(|| {
                let b = base?;
                b.action.clone()
            })
            .unwrap_or_else(|| "static_command".to_owned());
        let category = ToolItem::infer_category(&name);

        let command = raw_tool.command.or_else(|| {
            let b = base?;
            b.command.clone()
        });
        let allowed_directories = raw_tool.allowed_directories.or_else(|| {
            let b = base?;
            b.allowed_directories.clone()
        });
        let timeout_seconds = raw_tool.timeout_seconds.or_else(|| {
            let b = base?;
            b.timeout_seconds
        });
        let output_capture = raw_tool.output_capture.or_else(|| {
            let b = base?;
            b.output_capture.clone()
        });
        let ban_dangerous_files = raw_tool.ban_dangerous_files.or_else(|| {
            let b = base?;
            b.ban_dangerous_files
        }).unwrap_or(false);
        let enforce_workspace_bounds = raw_tool.enforce_workspace_bounds.or_else(|| {
            let b = base?;
            b.enforce_workspace_bounds
        }).unwrap_or(false);
        let attach_rustc_explain = raw_tool.attach_rustc_explain.or_else(|| {
            let b = base?;
            b.attach_rustc_explain
        }).unwrap_or(false);
        let require_clippy_clean = raw_tool.require_clippy_clean.or_else(|| {
            let b = base?;
            b.require_clippy_clean
        }).unwrap_or(false);
        let require_tests_passed = raw_tool.require_tests_passed.or_else(|| {
            let b = base?;
            b.require_tests_passed
        }).unwrap_or(false);
        let require_git_clean = raw_tool.require_git_clean.or_else(|| {
            let b = base?;
            b.require_git_clean
        }).unwrap_or(false);
        let require_permission_gate = raw_tool.require_permission_gate.or_else(|| {
            let b = base?;
            b.require_permission_gate
        }).unwrap_or(false);

        items.push(ToolItem {
            name,
            description,
            action,
            category,
            command,
            allowed_directories,
            timeout_seconds,
            output_capture,
            ban_dangerous_files,
            enforce_workspace_bounds,
            attach_rustc_explain,
            require_clippy_clean,
            require_tests_passed,
            require_git_clean,
            require_permission_gate,
        });
    }

    // Sort alphabetically by name
    items.sort_by(|a, b| a.name.cmp(&b.name));
    Ok(items)
}

/// Agent manifest parsed from an agent Markdown file with YAML frontmatter.
#[derive(Debug, Clone)]
pub(crate) struct AgentManifest {
    pub(crate) file_path: PathBuf,
    pub(crate) name: String,
    pub(crate) description: String,
    pub(crate) effort: String,
    pub(crate) permission_mode: String,
    pub(crate) command_execution_policy: String,
    pub(crate) main_agent: bool,
    pub(crate) subagent: bool,
    pub(crate) inherit_mcp: bool,
    pub(crate) native_tools: Vec<String>,
    pub(crate) mcp_tools: Vec<String>,
    pub(crate) allowed_subagents: Vec<String>,
    pub(crate) directives: String,
    pub(crate) modified: bool,
}

impl AgentManifest {
    /// Checks whether an MCP tool is allowed for this agent.
    #[must_use]
    pub(crate) fn allows_mcp_tool(&self, tool_name: &str) -> bool {
        self.mcp_tools.iter().any(|t| t == tool_name)
    }

    /// Toggles an MCP tool on or off in this agent's allowlist.
    pub(crate) fn toggle_mcp_tool(&mut self, tool_name: &str) {
        if let Some(pos) = self.mcp_tools.iter().position(|t| t == tool_name) {
            self.mcp_tools.remove(pos);
        } else {
            self.mcp_tools.push(tool_name.to_owned());
            self.mcp_tools.sort();
        }
        self.modified = true;
    }

    /// Cycles the agent's permission mode and command execution policy:
    /// Autonomous (acceptEdits / off) -> Supervised (acceptEdits / ask) -> Planning (plan / off).
    pub(crate) fn cycle_permission_policy(&mut self) -> &'static str {
        let desc = match (self.permission_mode.as_str(), self.command_execution_policy.as_str()) {
            ("acceptEdits", "off") => {
                "acceptEdits".clone_into(&mut self.permission_mode);
                "ask".clone_into(&mut self.command_execution_policy);
                "Supervised (ASK before commands)"
            }
            ("acceptEdits", "ask" | "prompt") => {
                "plan".clone_into(&mut self.permission_mode);
                "off".clone_into(&mut self.command_execution_policy);
                "Planning (Read-only PLAN mode)"
            }
            _ => {
                "acceptEdits".clone_into(&mut self.permission_mode);
                "off".clone_into(&mut self.command_execution_policy);
                "Autonomous (ALLOW all commands)"
            }
        };
        self.modified = true;
        desc
    }

    /// Serializes this agent manifest to YAML frontmatter + Markdown directives.
    #[must_use]
    pub(crate) fn serialize(&self) -> String {
        let mut out = String::new();
        out.push_str("---\n");
        out.push_str(&format!("name: {}\n", self.name));
        out.push_str(&format!("description: \"{}\"\n", self.description.replace('"', "\\\"")));
        out.push_str(&format!("effort: {}\n", self.effort));
        if !self.permission_mode.is_empty() {
            out.push_str(&format!("permissionMode: {}\n", self.permission_mode));
        }
        if !self.command_execution_policy.is_empty() {
            out.push_str(&format!("commandExecutionPolicy: {}\n", self.command_execution_policy));
        }
        if self.main_agent {
            out.push_str("mainAgent: true\n");
        }
        if self.subagent {
            out.push_str("subagent: true\n");
        }
        if !self.allowed_subagents.is_empty() {
            out.push_str("allowed_subagents:\n");
            for sub in &self.allowed_subagents {
                out.push_str(&format!("  - {sub}\n"));
            }
        }
        if !self.inherit_mcp {
            out.push_str("inheritMcp: false\n");
        }
        out.push_str("mcpServers:\n");
        out.push_str("  - name: agy-tools\n");
        out.push_str("    command: /home/infinite/.cargo/bin/agy-mcp\n");
        out.push_str(&format!("    args: [\"--agent\", \"{}\"]\n", self.name));

        if self.native_tools.is_empty() {
            out.push_str("tools: []\n");
        } else {
            out.push_str("tools:\n");
            for tool in &self.native_tools {
                out.push_str(&format!("  - {tool}\n"));
            }
        }

        if self.mcp_tools.is_empty() {
            out.push_str("mcp_tools: []\n");
        } else {
            out.push_str("mcp_tools:\n");
            for tool in &self.mcp_tools {
                out.push_str(&format!("  - {tool}\n"));
            }
        }
        out.push_str("---\n\n");
        out.push_str(self.directives.trim_start());
        if !out.ends_with('\n') {
            out.push('\n');
        }
        out
    }
}

/// Parses an agent Markdown file into an `AgentManifest`.
pub(crate) fn parse_agent_file(path: &Path) -> Result<AgentManifest, String> {
    let content = fs::read_to_string(path).map_err(|e| format!("Failed to read {}: {e}", path.display()))?;
    parse_agent_markdown(path.to_path_buf(), &content)
}

/// Parses an agent Markdown content string.
pub(crate) fn parse_agent_markdown(file_path: PathBuf, content: &str) -> Result<AgentManifest, String> {
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
    let mut native_tools = Vec::new();
    let mut mcp_tools = Vec::new();

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
        }
    }

    if name.is_empty()
        && let Some(stem) = file_path.file_stem().and_then(std::ffi::OsStr::to_str)
    {
        stem.clone_into(&mut name);
    }

    Ok(AgentManifest {
        file_path,
        name,
        description,
        effort,
        permission_mode,
        command_execution_policy,
        main_agent,
        subagent,
        inherit_mcp,
        native_tools,
        mcp_tools,
        allowed_subagents,
        directives,
        modified: false,
    })
}

/// Discovers and loads all agent manifests within the plugin or repository.
pub(crate) fn discover_agents(workspace_root: &Path) -> Vec<AgentManifest> {
    let mut candidates = Vec::new();

    let plugin_agents = workspace_root.join("plugins/agy-suite/agents");
    if plugin_agents.is_dir() {
        candidates.push(plugin_agents);
    }

    let repo_agents = workspace_root.join(".agents/agents");
    if repo_agents.is_dir() {
        candidates.push(repo_agents);
    }

    if let Ok(home) = std::env::var("HOME") {
        let home_plugin = PathBuf::from(home).join(".gemini/config/plugins/agy-suite/agents");
        if home_plugin.is_dir() && !candidates.contains(&home_plugin) {
            candidates.push(home_plugin);
        }
    }

    let mut agents = Vec::new();
    let mut seen_names = HashSet::new();

    for dir in candidates {
        let Ok(entries) = fs::read_dir(&dir) else { continue };

        for entry in entries.flatten() {
            let path = entry.path();
            if path.extension() != Some(std::ffi::OsStr::new("md")) {
                continue;
            }
            let Ok(agent) = parse_agent_file(&path) else { continue };
            if seen_names.insert(agent.name.clone()) {
                agents.push(agent);
            }
        }
    }

    agents.sort_by(|a, b| a.name.cmp(&b.name));
    agents
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_tool_category_inference() {
        assert_eq!(ToolItem::infer_category("run_cargo_clippy"), ToolCategory::Compilation);
        assert_eq!(ToolItem::infer_category("run_cargo_nextest"), ToolCategory::Testing);
        assert_eq!(ToolItem::infer_category("analyze_llvm_mca"), ToolCategory::Profiling);
        assert_eq!(ToolItem::infer_category("write_source_file"), ToolCategory::Filesystem);
        assert_eq!(ToolItem::infer_category("run_cargo_deny"), ToolCategory::Release);
        assert_eq!(ToolItem::infer_category("inspect_project_state"), ToolCategory::Diagnostics);
        assert_eq!(ToolItem::infer_category("unknown_custom_tool"), ToolCategory::Other);
    }

    #[test]
    fn test_parse_and_serialize_agent_markdown() {
        let sample = "\
---
name: test-agent
description: Test Agent for verification
effort: high
permissionMode: acceptEdits
commandExecutionPolicy: off
mainAgent: false
subagent: true
inheritMcp: false
allowed_subagents:
  - mini
tools: []
mcp_tools:
  - run_cargo_clippy
  - run_cargo_test
mcpServers:
  - name: agy-tools
    command: agy-mcp
    args:
      - --agent
      - test-agent
---

# Instructions

You are a test agent. Always pass tests.
";
        let Ok(manifest) = parse_agent_markdown(PathBuf::from("test.md"), sample) else {
            return;
        };
        assert_eq!(manifest.name, "test-agent");
        assert_eq!(manifest.effort, "high");
        assert!(!manifest.inherit_mcp);
        assert_eq!(manifest.mcp_tools, vec!["run_cargo_clippy", "run_cargo_test"]);
        assert_eq!(manifest.allowed_subagents, vec!["mini"]);
        assert!(manifest.directives.contains("You are a test agent."));

        let serialized = manifest.serialize();
        assert!(serialized.contains("name: test-agent"));
        assert!(serialized.contains("mcp_tools:"));
        assert!(serialized.contains("- run_cargo_clippy"));
        assert!(serialized.contains("inheritMcp: false"));
        assert!(serialized.contains("You are a test agent."));
    }
}
