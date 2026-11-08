//! Data models, YAML frontmatter parser/serializer, and tool catalog definitions.

use std::{
    fmt::Write,
    fs,
    path::{Path, PathBuf},
};

use agy_common::{
    FastHashSet,
    tools::{
        RawToolsConfig, ToolAction, ToolDefinition, ToolFormatting, ToolPreconditions,
        ToolSafetyPolicy, ToolSandboxPolicy, resolve,
    },
};

/// Category of an MCP tool for filtering and visual grouping.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub(crate) enum ToolCategory {
    /// Compilation & code verification (compilers, linters, formatters)
    Compilation,
    /// Automated testing, coverage & fuzzing (`test`, `coverage`, `mutants`, `fuzz`)
    Testing,
    /// Hardware performance, microarchitecture, benchmarks & profiling (`asm`, `perf`, `flamegraph`)
    Profiling,
    /// Transactional, bounded filesystem operations (`read_project_file`, `mpatch`)
    Filesystem,
    /// Supply-chain, licensing, `SemVer` & changelog auditing (`licenses`, `semver`)
    Release,
    /// Repository state, static analysis, specs & manuals (`ast_grep`, `graphtrail`, `tldr`)
    Diagnostics,
    /// Documentation generation, markdown and spec queries (`cargo_doc`, `git_cliff`)
    Documentation,
    /// Version control operations (`jj diff`, `jj absorb`, `git status`)
    Vcs,
    /// Isolated subagent workspaces (`ensure_agent_workspace`, `merge_agent_workspace`)
    Workspace,
    /// System health, supervisor and circuit breaker controls (`inspect_breaker`, `reset_breaker`)
    Management,
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
            Self::Documentation => "Documentation & Specs",
            Self::Vcs => "Version Control (JJ/Git)",
            Self::Workspace => "Subagent Workspaces",
            Self::Management => "Supervisor & Circuit Breaker",
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
            "documentation" => Ok(Self::Documentation),
            "vcs" => Ok(Self::Vcs),
            "workspace" => Ok(Self::Workspace),
            "management" => Ok(Self::Management),
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
                ToolAction::CliManual => "cli_manual".to_owned(),
                ToolAction::ProjectState => "project_state".to_owned(),
                ToolAction::WorkspaceEnsure => "workspace_ensure".to_owned(),
                ToolAction::WorkspaceInspect => "workspace_inspect".to_owned(),
                ToolAction::WorkspaceMerge => "workspace_merge".to_owned(),
                ToolAction::Unknown(s) => s,
            },
            category,
            safety: def.safety,
            sandbox: def.sandbox,
            preconditions: def.preconditions,
            formatting: def.formatting,
            command: if def.command.is_empty() {
                None
            } else {
                Some(def.command)
            },
            allowed_directories: if def.allowed_directories.is_empty() {
                None
            } else {
                Some(def.allowed_directories)
            },
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
    pub(crate) fn required_gates(&self) -> &[String] {
        &self.preconditions.require_gates
    }

    #[must_use]
    pub(crate) const fn emit_truncation_notice(&self) -> bool {
        self.formatting.emit_truncation_notice
    }

    /// Infers the logical category based on the tool name and action using generic semantic keywords.
    #[must_use]
    pub(crate) fn infer_category(name: &str) -> ToolCategory {
        let n = name.to_ascii_lowercase();
        if n.contains("test")
            || n.contains("fuzz")
            || n.contains("mutation")
            || n.contains("mutant")
            || n.contains("insta")
        {
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
        } else if n.contains("workspace") {
            ToolCategory::Workspace
        } else if n.contains("breaker") {
            ToolCategory::Management
        } else if n.contains("diff")
            || n.contains("bookmark")
            || n.contains("absorb")
            || n.contains("describe")
            || n.contains("vcs")
        {
            ToolCategory::Vcs
        } else if n.contains("doc")
            || n.contains("cliff")
            || n.contains("_spec")
            || n.starts_with("spec")
        {
            ToolCategory::Documentation
        } else if n.contains("release")
            || n.contains("deny")
            || n.contains("semver")
            || n.contains("publish")
            || n.contains("machete")
            || n.contains("outdated")
            || n.contains("public_api")
            || n.contains("hack")
        {
            ToolCategory::Release
        } else if n.contains("search")
            || n.contains("read")
            || n.contains("write")
            || n.contains("modify")
            || n.contains("patch")
        {
            ToolCategory::Filesystem
        } else if n.contains("inspect")
            || n.contains("query")
            || n.contains("explain")
            || n.contains("status")
            || n.contains("math")
            || n.contains("grep")
            || n.contains("log")
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
    let content =
        fs::read_to_string(path).map_err(|e| format!("Failed to read {}: {e}", path.display()))?;
    let raw: RawToolsConfig =
        toml::from_str(&content).map_err(|e| format!("Failed to parse {}: {e}", path.display()))?;
    let defs = resolve(&raw);
    let mut items: Vec<ToolItem> = defs.into_iter().map(ToolItem::from_definition).collect();
    items.sort_by(|a, b| a.name.cmp(&b.name));
    Ok(items)
}

/// Updates or removes `require_permission_gate = true` for the specified tool in a `tools.toml` file.
///
/// # Errors
/// Returns an error if reading, parsing, or writing the file fails.
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

    let mut doc = content
        .parse::<toml_edit::DocumentMut>()
        .map_err(|e| format!("Failed to parse {}: {e}", file_path.display()))?;

    if let Some(tools) = doc
        .get_mut("tools")
        .and_then(toml_edit::Item::as_table_like_mut)
        && let Some(tool_item) = tools
            .get_mut(tool_name)
            .and_then(toml_edit::Item::as_table_like_mut)
    {
        if require_gate {
            tool_item.insert("require_permission_gate", toml_edit::value(true));
        } else {
            tool_item.remove("require_permission_gate");
        }
    }

    fs::write(file_path, doc.to_string())
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
    pub(crate) allowed_dirs: Vec<String>,
    pub(crate) disallowed_dirs: Vec<String>,
    pub(crate) allowed_files: Vec<String>,
    pub(crate) disallowed_files: Vec<String>,
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
        let desc = match (
            self.permission_mode.as_str(),
            self.command_execution_policy.as_str(),
        ) {
            ("acceptEdits", "off") => {
                "acceptEdits".clone_into(&mut self.permission_mode);
                "ask".clone_into(&mut self.command_execution_policy);
                "Supervised (ASK before commands)"
            },
            ("acceptEdits", "ask" | "prompt") => {
                "plan".clone_into(&mut self.permission_mode);
                "off".clone_into(&mut self.command_execution_policy);
                "Planning (Read-only PLAN mode)"
            },
            _ => {
                "acceptEdits".clone_into(&mut self.permission_mode);
                "off".clone_into(&mut self.command_execution_policy);
                "Autonomous (ALLOW all commands)"
            },
        };
        self.modified = true;
        desc
    }

    /// Serializes this agent manifest to YAML frontmatter + Markdown directives.
    #[must_use]
    pub(crate) fn serialize(&self) -> String {
        let mut out = String::new();
        out.push_str("---\n");
        _ = writeln!(out, "name: {}", self.name);
        _ = writeln!(
            out,
            "description: \"{}\"",
            self.description.replace('"', "\\\"")
        );
        _ = writeln!(out, "effort: {}", self.effort);
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
        if self.role_flags.main_agent {
            out.push_str("mainAgent: true\n");
        }
        if self.role_flags.subagent {
            out.push_str("subagent: true\n");
        }
        if !self.allowed_subagents.is_empty() {
            out.push_str("allowed_subagents:\n");
            for sub in &self.allowed_subagents {
                _ = writeln!(out, "  - {sub}");
            }
        }
        if !self.pre_launch_checks.is_empty() {
            out.push_str("pre_launch_checks:\n");
            for check in &self.pre_launch_checks {
                _ = writeln!(out, "  - {check}");
            }
        }
        if !self.allowed_dirs.is_empty() {
            out.push_str("allowed_dirs:\n");
            for dir in &self.allowed_dirs {
                _ = writeln!(out, "  - {dir}");
            }
        }
        if !self.disallowed_dirs.is_empty() {
            out.push_str("disallowed_dirs:\n");
            for dir in &self.disallowed_dirs {
                _ = writeln!(out, "  - {dir}");
            }
        }
        if !self.allowed_files.is_empty() {
            out.push_str("allowed_files:\n");
            for file in &self.allowed_files {
                _ = writeln!(out, "  - {file}");
            }
        }
        if !self.disallowed_files.is_empty() {
            out.push_str("disallowed_files:\n");
            for file in &self.disallowed_files {
                _ = writeln!(out, "  - {file}");
            }
        }
        if !self.inherit_mcp {
            out.push_str("inheritMcp: false\n");
        }
        out.push_str("mcpServers:\n");
        out.push_str("  - name: agy-tools\n");
        out.push_str("    command: agy-mcp\n");
        _ = writeln!(out, "    args: [\"--agent\", \"{}\"]", self.name);

        if self.native_tools.is_empty() {
            out.push_str("tools: []\n");
        } else {
            out.push_str("tools:\n");
            for tool in &self.native_tools {
                _ = writeln!(out, "  - {tool}");
            }
        }

        if self.mcp_tools.is_empty() {
            out.push_str("mcp_tools: []\n");
        } else {
            out.push_str("mcp_tools:\n");
            for tool in &self.mcp_tools {
                _ = writeln!(out, "  - {tool}");
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
    let content =
        fs::read_to_string(path).map_err(|e| format!("Failed to read {}: {e}", path.display()))?;
    parse_agent_markdown(path.to_path_buf(), &content)
}

/// Parses an agent Markdown content string.
pub(crate) fn parse_agent_markdown(
    file_path: PathBuf,
    content: &str,
) -> Result<AgentManifest, String> {
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
        allowed_dirs: parsed.allowed_dirs,
        disallowed_dirs: parsed.disallowed_dirs,
        allowed_files: parsed.allowed_files,
        disallowed_files: parsed.disallowed_files,
        directives: parsed.directives,
        modified: false,
    })
}

/// Discovers and loads all agent manifests within the plugin or repository.
pub(crate) fn discover_agents(workspace_root: &Path) -> Vec<AgentManifest> {
    let candidate_dirs = agy_common::frontmatter::candidate_agent_directories(workspace_root);
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
            let Ok(agent) = parse_agent_file(&path) else {
                continue;
            };
            if seen_names.insert(agent.name.clone()) {
                agents.push(agent);
            }
        }
    }

    agents.sort_by(|a, b| a.name.cmp(&b.name));
    agents
}

#[cfg(test)]
#[allow(clippy::unwrap_used, reason = "Unit test assertions")]
mod tests {
    use super::*;

    #[test]
    fn test_tool_category_inference() {
        assert_eq!(
            ToolItem::infer_category("cargo_clippy"),
            ToolCategory::Compilation
        );
        assert_eq!(
            ToolItem::infer_category("cargo_nextest"),
            ToolCategory::Testing
        );
        assert_eq!(
            ToolItem::infer_category("llvm_mca"),
            ToolCategory::Profiling
        );
        assert_eq!(
            ToolItem::infer_category("write_file"),
            ToolCategory::Filesystem
        );
        assert_eq!(
            ToolItem::infer_category("modify_code"),
            ToolCategory::Filesystem
        );
        assert_eq!(
            ToolItem::infer_category("cargo_deny"),
            ToolCategory::Release
        );
        assert_eq!(
            ToolItem::infer_category("inspect_project_state"),
            ToolCategory::Diagnostics
        );
        assert_eq!(
            ToolItem::infer_category("unknown_custom_tool"),
            ToolCategory::Other
        );
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
  - helper_agent
pre_launch_checks:
  - gate_a
  - gate_b
tools: []
mcp_tools:
  - tool_a
  - tool_b
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
        assert_eq!(manifest.mcp_tools, vec!["tool_a", "tool_b"]);
        assert_eq!(manifest.allowed_subagents, vec!["helper_agent"]);
        assert_eq!(manifest.pre_launch_checks, vec!["gate_a", "gate_b"]);
        assert!(manifest.directives.contains("You are a test agent."));

        let serialized = manifest.serialize();
        assert!(serialized.contains("name: test-agent"));
        assert!(serialized.contains("pre_launch_checks:"));
        assert!(serialized.contains("- gate_a"));
        assert!(serialized.contains("mcp_tools:"));
        assert!(serialized.contains("- tool_a"));
        assert!(serialized.contains("inheritMcp: false"));
        assert!(serialized.contains("You are a test agent."));
    }

    #[test]
    fn test_update_tool_permission_gate_in_file() {
        let temp_dir = std::env::temp_dir().join(format!("agy_test_gate_{}", std::process::id()));
        drop(fs::create_dir_all(&temp_dir));
        let toml_file = temp_dir.join("tools.toml");

        let initial = "# Initial tools configuration\n[tools.run_tests]\n# Important comment to preserve\ndescription = \"Run tests\"\ncommand = [\"cargo\", \"test\"]\n";
        fs::write(&toml_file, initial).unwrap();

        // 1. Enable gate
        update_tool_permission_gate_in_file(&toml_file, "run_tests", true).unwrap();
        let content_enabled = fs::read_to_string(&toml_file).unwrap();
        assert!(content_enabled.contains("require_permission_gate = true"));
        assert!(content_enabled.contains("# Important comment to preserve"));

        // 2. Disable gate
        update_tool_permission_gate_in_file(&toml_file, "run_tests", false).unwrap();
        let content_disabled = fs::read_to_string(&toml_file).unwrap();
        assert!(!content_disabled.contains("require_permission_gate = true"));
        assert!(content_disabled.contains("# Important comment to preserve"));

        drop(fs::remove_dir_all(&temp_dir));
    }
}
