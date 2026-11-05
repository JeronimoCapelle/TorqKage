//! Data models, YAML frontmatter parser/serializer, and tool catalog definitions.

use std::collections::HashSet;
use std::fs;
use std::path::{Path, PathBuf};
use agy_common::tools::{
    RawToolsConfig, ToolAction, ToolDefinition, ToolFormatting, ToolPreconditions,
    ToolSafetyPolicy, ToolSandboxPolicy, resolve_tools,
};

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

impl std::str::FromStr for ToolCategory {
    type Err = ();

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s.to_ascii_lowercase().as_str() {
            "compilation" => Ok(Self::Compilation),
            "testing" => Ok(Self::Testing),
            "profiling" => Ok(Self::Profiling),
            "filesystem" => Ok(Self::Filesystem),
            "release" => Ok(Self::Release),
            "diagnostics" => Ok(Self::Diagnostics),
            _ => Ok(Self::Other),
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
    pub(crate) safety: ToolSafetyPolicy,
    pub(crate) sandbox: ToolSandboxPolicy,
    pub(crate) preconditions: ToolPreconditions,
    pub(crate) formatting: ToolFormatting,
    pub(crate) command: Option<Vec<String>>,
    pub(crate) allowed_directories: Option<Vec<String>>,
    pub(crate) timeout_seconds: Option<u64>,
    pub(crate) output_capture: Option<String>,
    pub(crate) modified: bool,
}

impl ToolItem {
    #[must_use]
    pub(crate) fn from_definition(def: ToolDefinition) -> Self {
        let category = def
            .category
            .as_deref()
            .and_then(|c| c.parse().ok())
            .unwrap_or_else(|| Self::infer_category(&def.name));

        Self {
            name: def.name,
            description: def.description,
            action: match def.action {
                ToolAction::StaticCommand => "static_command".to_owned(),
                ToolAction::ParameterizedCommand => "parameterized_command".to_owned(),
                ToolAction::FileRead => "file_read".to_owned(),
                ToolAction::FileWrite => "file_write".to_owned(),
                ToolAction::MpatchEdit => "mpatch_edit".to_owned(),
                ToolAction::FileSearch => "file_search".to_owned(),
                ToolAction::CargoJsonDiagnostics => "cargo_json_diagnostics".to_owned(),
                ToolAction::CliManual => "cli_manual".to_owned(),
                ToolAction::ProjectState => "project_state".to_owned(),
                ToolAction::WorkspaceEnsure => "workspace_ensure".to_owned(),
                ToolAction::Unknown(s) => s,
            },
            category,
            safety: def.safety,
            sandbox: def.sandbox,
            preconditions: def.preconditions,
            formatting: def.formatting,
            command: if def.command.is_empty() { None } else { Some(def.command) },
            allowed_directories: if def.allowed_directories.is_empty() { None } else { Some(def.allowed_directories) },
            timeout_seconds: Some(def.timeout_seconds),
            output_capture: Some(format!("{:?}", def.output_capture).to_lowercase()),
            modified: false,
        }
    }

    #[must_use]
    pub(crate) const fn mutating(&self) -> bool {
        self.safety.mutating
    }

    #[must_use]
    pub(crate) const fn tracks_circuit_breaker(&self) -> bool {
        self.safety.tracks_circuit_breaker
    }

    #[must_use]
    pub(crate) const fn require_permission_gate(&self) -> bool {
        self.safety.require_permission_gate
    }

    #[must_use]
    pub(crate) const fn enforce_sandbox(&self) -> bool {
        self.sandbox.enforce_sandbox
    }

    #[must_use]
    pub(crate) const fn enforce_workspace_bounds(&self) -> bool {
        self.sandbox.enforce_workspace_bounds
    }

    #[must_use]
    pub(crate) const fn ban_dangerous_files(&self) -> bool {
        self.sandbox.ban_dangerous_files
    }

    #[must_use]
    pub(crate) const fn require_git_clean(&self) -> bool {
        self.preconditions.require_git_clean
    }

    #[must_use]
    pub(crate) const fn require_clippy_clean(&self) -> bool {
        self.preconditions.require_clippy_clean
    }

    #[must_use]
    pub(crate) const fn require_tests_passed(&self) -> bool {
        self.preconditions.require_tests_passed
    }

    #[must_use]
    pub(crate) const fn attach_rustc_explain(&self) -> bool {
        self.formatting.attach_rustc_explain
    }

    /// Infers the logical category based on the tool name and action using generic semantic keywords.
    #[must_use]
    pub(crate) fn infer_category(name: &str) -> ToolCategory {
        let n = name.to_ascii_lowercase();
        if n.contains("test") || n.contains("fuzz") || n.contains("mutation") || n.contains("insta") {
            ToolCategory::Testing
        } else if n.contains("bench")
            || n.contains("profile")
            || n.contains("perf")
            || n.contains("bloat")
            || n.contains("flamegraph")
            || n.contains("hyperfine")
            || n.contains("asm")
            || n.contains("llvm")
            || n.contains("miri")
            || n.contains("careful")
        {
            ToolCategory::Profiling
        } else if n.contains("check")
            || n.contains("build")
            || n.contains("compile")
            || n.contains("clippy")
            || n.contains("lint")
            || n.contains("fmt")
            || n.contains("expand")
            || n.contains("verification")
        {
            ToolCategory::Compilation
        } else if n.contains("release")
            || n.contains("deny")
            || n.contains("semver")
            || n.contains("cliff")
            || n.contains("publish")
            || n.contains("machete")
            || n.contains("outdated")
            || n.contains("public_api")
            || n.contains("hack")
        {
            ToolCategory::Release
        } else if n.contains("search") || n.contains("read") || n.contains("write") || n.contains("modify") || n.contains("patch") {
            ToolCategory::Filesystem
        } else if n.contains("inspect")
            || n.contains("query")
            || n.contains("explain")
            || n.contains("status")
            || n.contains("math")
            || n.contains("grep")
            || n.contains("diff")
            || n.contains("log")
            || n.contains("bookmark")
            || n.contains("absorb")
            || n.contains("describe")
        {
            ToolCategory::Diagnostics
        } else {
            ToolCategory::Other
        }
    }
}

/// Loads all tools from a `tools.toml` file path.
///
/// # Errors
/// Returns an error if reading or parsing the file fails.
pub(crate) fn load_tools_from_file(path: &Path) -> Result<Vec<ToolItem>, String> {
    let content = fs::read_to_string(path).map_err(|e| format!("Failed to read {}: {e}", path.display()))?;
    let raw: RawToolsConfig = toml::from_str(&content).map_err(|e| format!("Failed to parse {}: {e}", path.display()))?;
    let defs = resolve_tools(&raw);
    let mut items: Vec<ToolItem> = defs.into_iter().map(ToolItem::from_definition).collect();
    items.sort_by(|a, b| a.name.cmp(&b.name));
    Ok(items)
}

/// Updates or removes `require_permission_gate = true` for the specified tool in a `tools.toml` file.
///
/// # Errors
/// Returns an error if reading or writing the file fails.
pub(crate) fn update_tool_permission_gate_in_file(
    file_path: &Path,
    tool_name: &str,
    require_gate: bool,
) -> Result<(), String> {
    if !file_path.is_file() {
        return Ok(());
    }
    let content = fs::read_to_string(file_path)
        .map_err(|e| format!("Failed to read {}: {e}", file_path.display()))?;

    let header = format!("[tools.{tool_name}]");
    let mut lines: Vec<String> = content.lines().map(ToOwned::to_owned).collect();
    let mut in_section = false;
    let mut found_gate_line = None;
    let mut section_start_idx = None;

    for (idx, line) in lines.iter().enumerate() {
        let trimmed = line.trim();
        if trimmed == header {
            in_section = true;
            section_start_idx = Some(idx);
            continue;
        }
        if in_section {
            if trimmed.starts_with('[') {
                break;
            }
            if trimmed.starts_with("require_permission_gate") {
                found_gate_line = Some(idx);
                break;
            }
        }
    }

    if let Some(gate_idx) = found_gate_line {
        if require_gate {
            if let Some(line) = lines.get_mut(gate_idx) {
                "require_permission_gate = true".clone_into(line);
            }
        } else if gate_idx < lines.len() {
            lines.remove(gate_idx);
        }
    } else if require_gate
        && let Some(start_idx) = section_start_idx
    {
        lines.insert(start_idx.saturating_add(1), "require_permission_gate = true".to_owned());
    }

    let mut new_content = lines.join("\n");
    if !new_content.ends_with('\n') {
        new_content.push('\n');
    }

    fs::write(file_path, new_content)
        .map_err(|e| format!("Failed to write {}: {e}", file_path.display()))?;

    Ok(())
}

/// Classification flags defining whether an agent is a root agent or a subagent.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub(crate) struct AgentRoleFlags {
    pub(crate) main_agent: bool,
    pub(crate) subagent: bool,
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
    pub(crate) role_flags: AgentRoleFlags,
    pub(crate) inherit_mcp: bool,
    pub(crate) native_tools: Vec<String>,
    pub(crate) mcp_tools: Vec<String>,
    pub(crate) allowed_subagents: Vec<String>,
    pub(crate) pre_launch_checks: Vec<String>,
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
        if self.role_flags.main_agent {
            out.push_str("mainAgent: true\n");
        }
        if self.role_flags.subagent {
            out.push_str("subagent: true\n");
        }
        if !self.allowed_subagents.is_empty() {
            out.push_str("allowed_subagents:\n");
            for sub in &self.allowed_subagents {
                out.push_str(&format!("  - {sub}\n"));
            }
        }
        if !self.pre_launch_checks.is_empty() {
            out.push_str("pre_launch_checks:\n");
            for check in &self.pre_launch_checks {
                out.push_str(&format!("  - {check}\n"));
            }
        }
        if !self.inherit_mcp {
            out.push_str("inheritMcp: false\n");
        }
        out.push_str("mcpServers:\n");
        out.push_str("  - name: agy-tools\n");
        out.push_str("    command: agy-mcp\n");
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
    let parsed = agy_common::frontmatter::AgentFrontmatter::parse(content)?;
    let mut name = parsed.name;
    if name.is_empty()
        && let Some(stem) = file_path.file_stem().and_then(std::ffi::OsStr::to_str)
    {
        stem.clone_into(&mut name);
    }

    Ok(AgentManifest {
        file_path,
        name,
        description: parsed.description,
        effort: parsed.effort,
        permission_mode: parsed.permission_mode,
        command_execution_policy: parsed.command_execution_policy,
        role_flags: AgentRoleFlags {
            main_agent: parsed.main_agent,
            subagent: parsed.subagent,
        },
        inherit_mcp: parsed.inherit_mcp,
        native_tools: parsed.native_tools,
        mcp_tools: parsed.mcp_tools,
        allowed_subagents: parsed.allowed_subagents,
        pre_launch_checks: parsed.pre_launch_checks,
        directives: parsed.directives,
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
        assert_eq!(ToolItem::infer_category("write_file"), ToolCategory::Filesystem);
        assert_eq!(ToolItem::infer_category("modify_code"), ToolCategory::Filesystem);
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
pre_launch_checks:
  - cargo_compilation
  - vcs_conflicts
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
        assert_eq!(manifest.pre_launch_checks, vec!["cargo_compilation", "vcs_conflicts"]);
        assert!(manifest.directives.contains("You are a test agent."));

        let serialized = manifest.serialize();
        assert!(serialized.contains("name: test-agent"));
        assert!(serialized.contains("pre_launch_checks:"));
        assert!(serialized.contains("- cargo_compilation"));
        assert!(serialized.contains("mcp_tools:"));
        assert!(serialized.contains("- run_cargo_clippy"));
        assert!(serialized.contains("inheritMcp: false"));
        assert!(serialized.contains("You are a test agent."));
    }
}
