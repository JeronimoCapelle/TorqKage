//! In-memory models and typed representation of declarative MCP tools.

use serde::{Deserialize, Serialize};

/// High-level action category executed by the MCP engine.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "snake_case")]
pub(crate) enum ToolAction {
    FileRead,
    FileWrite,
    MpatchEdit,
    FileSearch,
    #[default]
    StaticCommand,
    ParameterizedCommand,
    CargoJsonDiagnostics,
    CliManual,
    #[serde(untagged)]
    Unknown(String),
}

/// Output filtering and capture mode for process stdout/stderr.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "snake_case")]
pub(crate) enum OutputCapture {
    #[default]
    Stdout,
    Stderr,
    Both,
    ExitCodeOnly,
    JsonSpans,
}

/// Declared argument for parameterized commands.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Default)]
pub(crate) struct ToolArg {
    pub(crate) name: String,
    #[serde(default)]
    pub(crate) description: String,
    #[serde(default)]
    pub(crate) required: bool,
    #[serde(default)]
    pub(crate) regex: Option<String>,
    #[serde(default)]
    pub(crate) flag: Option<String>,
}

/// Fully-resolved declarative tool definition ready for JSON Schema generation and execution.
#[allow(
    clippy::struct_excessive_bools,
    reason = "Declarative tools.toml schema maps multiple boolean security policies"
)]
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Default)]
pub(crate) struct ToolDefinition {
    pub(crate) name: String,
    pub(crate) description: String,
    pub(crate) action: ToolAction,
    #[serde(default)]
    pub(crate) command: Vec<String>,
    #[serde(default)]
    pub(crate) args: Vec<ToolArg>,
    #[serde(default)]
    pub(crate) output_capture: OutputCapture,
    #[serde(default)]
    pub(crate) allowed_directories: Vec<String>,
    #[serde(default)]
    pub(crate) enforce_dir: Option<String>,
    #[serde(default)]
    pub(crate) enforce_file_target: Option<String>,
    #[serde(default)]
    pub(crate) enforce_workspace_bounds: bool,
    #[serde(default)]
    pub(crate) ban_dangerous_files: bool,
    #[serde(default)]
    pub(crate) sanitize_input: Option<String>,
    #[serde(default)]
    pub(crate) truncate_lines: Option<usize>,
    #[serde(default)]
    pub(crate) truncate_bytes: Option<usize>,
    #[serde(default)]
    pub(crate) emit_truncation_notice: bool,
    #[serde(default)]
    pub(crate) attach_rustc_explain: bool,
    #[serde(default = "default_timeout_seconds")]
    pub(crate) timeout_seconds: u64,
    #[serde(default)]
    pub(crate) require_permission_gate: bool,
    #[serde(default)]
    pub(crate) require_git_clean: bool,
    #[serde(default)]
    pub(crate) require_clippy_clean: bool,
    #[serde(default)]
    pub(crate) require_tests_passed: bool,
    #[serde(default)]
    pub(crate) error_fallback: Option<String>,
}

/// Declarative path and directory guards scoped to a specific agent role.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Default)]
pub(crate) struct AgentGuard {
    #[serde(default)]
    pub(crate) allowed_dirs: Vec<String>,
    #[serde(default)]
    pub(crate) disallowed_dirs: Vec<String>,
    #[serde(default)]
    pub(crate) allowed_files: Vec<String>,
    #[serde(default)]
    pub(crate) disallowed_files: Vec<String>,
}

const fn default_timeout_seconds() -> u64 {
    30
}

impl ToolDefinition {
    /// Returns the primary binary name invoked by this tool, if it executes an external command.
    #[must_use]
    pub(crate) fn primary_binary(&self) -> Option<&str> {
        self.command.first().map(String::as_str)
    }
}
