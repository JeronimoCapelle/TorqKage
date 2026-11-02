//! Declarative tool data types, safety policies, and inheritance merge engine.

use std::collections::BTreeMap;
use serde::{Deserialize, Serialize};

/// Action type determining the execution engine for a tool.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "snake_case")]
pub enum ToolAction {
    FileRead,
    FileWrite,
    MpatchEdit,
    FileSearch,
    #[default]
    StaticCommand,
    ParameterizedCommand,
    CargoJsonDiagnostics,
    CliManual,
    ProjectState,
    WorkspaceEnsure,
    #[serde(untagged)]
    Unknown(String),
}

/// Capture strategy for standard process streams.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "snake_case")]
pub enum OutputCapture {
    #[default]
    Stdout,
    Stderr,
    Both,
    ExitCodeOnly,
    JsonSpans,
}

/// Declared argument for parameterized commands.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Default)]
pub struct ToolArg {
    pub name: String,
    #[serde(default)]
    pub description: String,
    #[serde(default)]
    pub required: bool,
    #[serde(default)]
    pub regex: Option<String>,
    #[serde(default)]
    pub flag: Option<String>,
    #[serde(default)]
    pub default: Option<String>,
}

/// Security policies governing tool mutability and permission gates.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
pub struct ToolSafetyPolicy {
    #[serde(default)]
    pub mutating: bool,
    #[serde(default)]
    pub tracks_circuit_breaker: bool,
    #[serde(default)]
    pub require_permission_gate: bool,
}

/// Sandbox and filesystem confinement boundaries.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
pub struct ToolSandboxPolicy {
    #[serde(default)]
    pub enforce_sandbox: bool,
    #[serde(default)]
    pub enforce_workspace_bounds: bool,
    #[serde(default)]
    pub ban_dangerous_files: bool,
}

/// Preconditions required before tool execution.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
pub struct ToolPreconditions {
    #[serde(default)]
    pub require_git_clean: bool,
    #[serde(default)]
    pub require_clippy_clean: bool,
    #[serde(default)]
    pub require_tests_passed: bool,
}

/// Output truncation and formatting directives.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
pub struct ToolFormatting {
    #[serde(default)]
    pub emit_truncation_notice: bool,
    #[serde(default)]
    pub attach_rustc_explain: bool,
}

const fn default_timeout_seconds() -> u64 {
    15
}

/// Raw representation of a tool entry parsed from TOML before inheritance resolution.
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct RawTool {
    pub inherit: Option<String>,
    pub description: Option<String>,
    pub action: Option<ToolAction>,
    pub category: Option<String>,
    pub mutating: Option<bool>,
    pub tracks_circuit_breaker: Option<bool>,
    pub command: Option<Vec<String>>,
    pub args: Option<Vec<ToolArg>>,
    pub output_capture: Option<OutputCapture>,
    pub allowed_directories: Option<Vec<String>>,
    pub enforce_dir: Option<String>,
    pub enforce_file_target: Option<String>,
    pub enforce_workspace_bounds: Option<bool>,
    pub ban_dangerous_files: Option<bool>,
    pub sanitize_input: Option<String>,
    pub truncate_lines: Option<usize>,
    pub truncate_bytes: Option<usize>,
    pub emit_truncation_notice: Option<bool>,
    pub attach_rustc_explain: Option<bool>,
    pub timeout_seconds: Option<u64>,
    pub require_permission_gate: Option<bool>,
    pub enforce_sandbox: Option<bool>,
    pub sandbox: Option<bool>,
    pub require_git_clean: Option<bool>,
    pub require_workspace_clean: Option<bool>,
    pub require_clippy_clean: Option<bool>,
    pub require_tests_passed: Option<bool>,
    pub error_fallback: Option<String>,
    pub empty_fallback: Option<String>,
    pub success_exit_codes: Option<Vec<i32>>,
}

/// Configuration container holding base tools and resolved tools.
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct RawToolsConfig {
    #[serde(default)]
    pub base_tools: BTreeMap<String, RawTool>,
    #[serde(default)]
    pub tools: BTreeMap<String, RawTool>,
}

/// Fully-resolved declarative tool definition ready for JSON Schema generation and execution.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Default)]
pub struct ToolDefinition {
    pub name: String,
    pub description: String,
    pub action: ToolAction,
    #[serde(default)]
    pub category: Option<String>,
    #[serde(flatten)]
    pub safety: ToolSafetyPolicy,
    #[serde(flatten)]
    pub sandbox: ToolSandboxPolicy,
    #[serde(flatten)]
    pub preconditions: ToolPreconditions,
    #[serde(flatten)]
    pub formatting: ToolFormatting,
    #[serde(default)]
    pub command: Vec<String>,
    #[serde(default)]
    pub args: Vec<ToolArg>,
    #[serde(default)]
    pub output_capture: OutputCapture,
    #[serde(default)]
    pub allowed_directories: Vec<String>,
    #[serde(default)]
    pub enforce_dir: Option<String>,
    #[serde(default)]
    pub enforce_file_target: Option<String>,
    #[serde(default)]
    pub sanitize_input: Option<String>,
    #[serde(default)]
    pub truncate_lines: Option<usize>,
    #[serde(default)]
    pub truncate_bytes: Option<usize>,
    #[serde(default = "default_timeout_seconds")]
    pub timeout_seconds: u64,
    #[serde(default)]
    pub error_fallback: Option<String>,
    #[serde(default)]
    pub empty_fallback: Option<String>,
    #[serde(default)]
    pub success_exit_codes: Vec<i32>,
}

impl ToolDefinition {
    #[must_use]
    pub const fn mutating(&self) -> bool {
        self.safety.mutating
    }

    #[must_use]
    pub const fn tracks_circuit_breaker(&self) -> bool {
        self.safety.tracks_circuit_breaker
    }

    #[must_use]
    pub const fn require_permission_gate(&self) -> bool {
        self.safety.require_permission_gate
    }

    #[must_use]
    pub const fn enforce_sandbox(&self) -> bool {
        self.sandbox.enforce_sandbox
    }

    #[must_use]
    pub const fn enforce_workspace_bounds(&self) -> bool {
        self.sandbox.enforce_workspace_bounds
    }

    #[must_use]
    pub const fn ban_dangerous_files(&self) -> bool {
        self.sandbox.ban_dangerous_files
    }

    #[must_use]
    pub const fn require_git_clean(&self) -> bool {
        self.preconditions.require_git_clean
    }

    #[must_use]
    pub const fn require_clippy_clean(&self) -> bool {
        self.preconditions.require_clippy_clean
    }

    #[must_use]
    pub const fn require_tests_passed(&self) -> bool {
        self.preconditions.require_tests_passed
    }

    #[must_use]
    pub const fn emit_truncation_notice(&self) -> bool {
        self.formatting.emit_truncation_notice
    }

    #[must_use]
    pub const fn attach_rustc_explain(&self) -> bool {
        self.formatting.attach_rustc_explain
    }

    #[must_use]
    pub fn primary_binary(&self) -> Option<&str> {
        self.command.first().map(String::as_str)
    }
}

/// Merges a raw tool definition with its optional base definition.
#[must_use]
pub fn merge_tool(name: String, base: Option<&RawTool>, raw: &RawTool) -> ToolDefinition {
    let safety = ToolSafetyPolicy {
        mutating: raw.mutating.or_else(|| base?.mutating).unwrap_or(false),
        tracks_circuit_breaker: raw.tracks_circuit_breaker.or_else(|| base?.tracks_circuit_breaker).unwrap_or(false),
        require_permission_gate: raw.require_permission_gate.or_else(|| base?.require_permission_gate).unwrap_or(false),
    };

    let sandbox = ToolSandboxPolicy {
        enforce_sandbox: raw.enforce_sandbox.or(raw.sandbox).or_else(|| base?.enforce_sandbox.or(base?.sandbox)).unwrap_or(false),
        enforce_workspace_bounds: raw.enforce_workspace_bounds.or_else(|| base?.enforce_workspace_bounds).unwrap_or(false),
        ban_dangerous_files: raw.ban_dangerous_files.or_else(|| base?.ban_dangerous_files).unwrap_or(false),
    };

    let preconditions = ToolPreconditions {
        require_git_clean: raw.require_git_clean.or(raw.require_workspace_clean).or_else(|| base?.require_git_clean.or(base?.require_workspace_clean)).unwrap_or(false),
        require_clippy_clean: raw.require_clippy_clean.or_else(|| base?.require_clippy_clean).unwrap_or(false),
        require_tests_passed: raw.require_tests_passed.or_else(|| base?.require_tests_passed).unwrap_or(false),
    };

    let formatting = ToolFormatting {
        emit_truncation_notice: raw.emit_truncation_notice.or_else(|| base?.emit_truncation_notice).unwrap_or(false),
        attach_rustc_explain: raw.attach_rustc_explain.or_else(|| base?.attach_rustc_explain).unwrap_or(false),
    };

    ToolDefinition {
        name,
        description: raw.description.clone().or_else(|| base?.description.clone()).unwrap_or_default(),
        action: raw.action.clone().or_else(|| base?.action.clone()).unwrap_or_default(),
        category: raw.category.clone().or_else(|| base?.category.clone()),
        safety,
        sandbox,
        preconditions,
        formatting,
        command: raw.command.clone().or_else(|| base?.command.clone()).unwrap_or_default(),
        args: raw.args.clone().or_else(|| base?.args.clone()).unwrap_or_default(),
        output_capture: raw.output_capture.or_else(|| base?.output_capture).unwrap_or_default(),
        allowed_directories: raw.allowed_directories.clone().or_else(|| base?.allowed_directories.clone()).unwrap_or_default(),
        enforce_dir: raw.enforce_dir.clone().or_else(|| base?.enforce_dir.clone()),
        enforce_file_target: raw.enforce_file_target.clone().or_else(|| base?.enforce_file_target.clone()),
        sanitize_input: raw.sanitize_input.clone().or_else(|| base?.sanitize_input.clone()),
        truncate_lines: raw.truncate_lines.or_else(|| base?.truncate_lines),
        truncate_bytes: raw.truncate_bytes.or_else(|| base?.truncate_bytes),
        timeout_seconds: raw.timeout_seconds.or_else(|| base?.timeout_seconds).unwrap_or(15),
        error_fallback: raw.error_fallback.clone().or_else(|| base?.error_fallback.clone()),
        empty_fallback: raw.empty_fallback.clone().or_else(|| base?.empty_fallback.clone()),
        success_exit_codes: raw.success_exit_codes.clone().or_else(|| base?.success_exit_codes.clone()).unwrap_or_default(),
    }
}

/// Resolves inheritance for all tools defined in `RawToolsConfig`.
#[must_use]
#[allow(clippy::module_name_repetitions, reason = "Explicit function name for clarity when imported unqualified")]
pub fn resolve_tools(config: &RawToolsConfig) -> Vec<ToolDefinition> {
    let mut resolved = Vec::with_capacity(config.tools.len());
    for (name, raw_tool) in &config.tools {
        let base = raw_tool.inherit.as_deref().and_then(|key| {
            let clean = key.strip_prefix("base_tools.").unwrap_or(key);
            config.base_tools.get(clean)
        });
        resolved.push(merge_tool(name.clone(), base, raw_tool));
    }
    resolved
}

#[cfg(test)]
#[allow(
    clippy::unwrap_used,
    clippy::expect_used,
    reason = "Unit test assertions"
)]
mod tests {
    use super::*;

    #[test]
    fn test_merge_tool_inheritance() {
        let base = RawTool {
            description: Some("Base description".to_owned()),
            mutating: Some(true),
            enforce_sandbox: Some(true),
            ..Default::default()
        };

        let raw = RawTool {
            description: Some("Override description".to_owned()),
            ..Default::default()
        };

        let merged = merge_tool("test_tool".to_owned(), Some(&base), &raw);
        assert_eq!(merged.name, "test_tool");
        assert_eq!(merged.description, "Override description");
        assert!(merged.mutating());
        assert!(merged.enforce_sandbox());
        assert!(!merged.require_git_clean());
    }
}
