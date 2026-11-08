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
    CliManual,
    ProjectState,
    WorkspaceEnsure,
    WorkspaceInspect,
    WorkspaceMerge,
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
    #[serde(alias = "merged")]
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
    #[serde(default)]
    pub switch: bool,
    #[serde(default, rename = "type", alias = "arg_type")]
    pub arg_type: Option<String>,
    #[serde(default, rename = "enum", alias = "values")]
    pub enum_values: Vec<String>,
    #[serde(default)]
    pub conflicts_with: Vec<String>,
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
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Default)]
pub struct ToolPreconditions {
    #[serde(default)]
    pub require_gates: Vec<String>,
}

/// Output truncation and formatting directives.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
pub struct ToolFormatting {
    #[serde(default)]
    pub emit_truncation_notice: bool,
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
    pub timeout_seconds: Option<u64>,
    pub require_permission_gate: Option<bool>,
    pub enforce_sandbox: Option<bool>,
    pub sandbox: Option<bool>,
    pub require_gates: Option<Vec<String>>,
    pub skip_gates: Option<Vec<String>>,
    pub error_fallback: Option<String>,
    pub empty_fallback: Option<String>,
    pub success_exit_codes: Option<Vec<i32>>,
}

/// Configuration container holding base tools, category defaults, and resolved tools.
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct RawToolsConfig {
    #[serde(default)]
    pub base_tools: BTreeMap<String, RawTool>,
    #[serde(default)]
    pub categories: BTreeMap<String, RawTool>,
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

    /// Resolves the list of quality gates required prior to tool execution.
    #[must_use]
    pub fn required_gates(&self) -> &[String] {
        &self.preconditions.require_gates
    }

    #[must_use]
    pub const fn emit_truncation_notice(&self) -> bool {
        self.formatting.emit_truncation_notice
    }

    #[must_use]
    pub fn primary_binary(&self) -> Option<&str> {
        self.command.first().map(String::as_str)
    }
}

fn resolve_fallback<T: Clone>(
    raw: Option<T>,
    base: Option<&RawTool>,
    category: Option<&RawTool>,
    selector: impl Fn(&RawTool) -> Option<T>,
) -> Option<T> {
    raw.or_else(|| selector(base?))
        .or_else(|| selector(category?))
}

fn resolve_safety(
    raw: &RawTool,
    base: Option<&RawTool>,
    category: Option<&RawTool>,
) -> ToolSafetyPolicy {
    ToolSafetyPolicy {
        mutating: resolve_fallback(raw.mutating, base, category, |t| t.mutating).unwrap_or(false),
        tracks_circuit_breaker: resolve_fallback(raw.tracks_circuit_breaker, base, category, |t| {
            t.tracks_circuit_breaker
        })
        .unwrap_or(false),
        require_permission_gate: resolve_fallback(
            raw.require_permission_gate,
            base,
            category,
            |t| t.require_permission_gate,
        )
        .unwrap_or(false),
    }
}

fn resolve_sandbox(
    raw: &RawTool,
    base: Option<&RawTool>,
    category: Option<&RawTool>,
) -> ToolSandboxPolicy {
    let enforce_sandbox = raw
        .enforce_sandbox
        .or(raw.sandbox)
        .or_else(|| base?.enforce_sandbox.or(base?.sandbox))
        .or_else(|| category?.enforce_sandbox.or(category?.sandbox))
        .unwrap_or(false);

    ToolSandboxPolicy {
        enforce_sandbox,
        enforce_workspace_bounds: resolve_fallback(
            raw.enforce_workspace_bounds,
            base,
            category,
            |t| t.enforce_workspace_bounds,
        )
        .unwrap_or(false),
        ban_dangerous_files: resolve_fallback(raw.ban_dangerous_files, base, category, |t| {
            t.ban_dangerous_files
        })
        .unwrap_or(false),
    }
}

fn resolve_preconditions(
    raw: &RawTool,
    base: Option<&RawTool>,
    category: Option<&RawTool>,
) -> ToolPreconditions {
    let mut gates = Vec::new();
    let sources = [category, base, Some(raw)];
    for source in sources.into_iter().flatten() {
        if let Some(ref source_gates) = source.require_gates {
            for g in source_gates {
                if !gates.contains(g) {
                    gates.push(g.clone());
                }
            }
        }
    }

    let mut skip_set = Vec::new();
    if let Some(b) = base
        && let Some(ref base_skips) = b.skip_gates
    {
        skip_set.extend(base_skips.clone());
    }
    if let Some(ref raw_skips) = raw.skip_gates {
        skip_set.extend(raw_skips.clone());
    }

    if skip_set
        .iter()
        .any(|s| s == "*" || s.eq_ignore_ascii_case("all"))
    {
        gates.clear();
    } else {
        gates.retain(|g| !skip_set.iter().any(|s| s.eq_ignore_ascii_case(g)));
    }

    ToolPreconditions {
        require_gates: gates,
    }
}

/// Merges a raw tool definition with its optional category defaults and base definition.
#[must_use]
pub fn merge_tool_with_category(
    name: String,
    category: Option<&RawTool>,
    base: Option<&RawTool>,
    raw: &RawTool,
) -> ToolDefinition {
    let safety = resolve_safety(raw, base, category);
    let sandbox = resolve_sandbox(raw, base, category);
    let preconditions = resolve_preconditions(raw, base, category);
    let formatting = ToolFormatting {
        emit_truncation_notice: resolve_fallback(raw.emit_truncation_notice, base, category, |t| {
            t.emit_truncation_notice
        })
        .unwrap_or(false),
    };

    ToolDefinition {
        name,
        description: resolve_fallback(raw.description.clone(), base, category, |t| {
            t.description.clone()
        })
        .unwrap_or_default(),
        action: resolve_fallback(raw.action.clone(), base, category, |t| t.action.clone())
            .unwrap_or_default(),
        category: raw.category.clone().or_else(|| base?.category.clone()),
        safety,
        sandbox,
        preconditions,
        formatting,
        command: resolve_fallback(raw.command.clone(), base, category, |t| t.command.clone())
            .unwrap_or_default(),
        args: resolve_fallback(raw.args.clone(), base, category, |t| t.args.clone())
            .unwrap_or_default(),
        output_capture: resolve_fallback(raw.output_capture, base, category, |t| t.output_capture)
            .unwrap_or_default(),
        allowed_directories: resolve_fallback(
            raw.allowed_directories.clone(),
            base,
            category,
            |t| t.allowed_directories.clone(),
        )
        .unwrap_or_default(),
        enforce_dir: resolve_fallback(raw.enforce_dir.clone(), base, category, |t| {
            t.enforce_dir.clone()
        }),
        enforce_file_target: resolve_fallback(
            raw.enforce_file_target.clone(),
            base,
            category,
            |t| t.enforce_file_target.clone(),
        ),
        sanitize_input: resolve_fallback(raw.sanitize_input.clone(), base, category, |t| {
            t.sanitize_input.clone()
        }),
        truncate_lines: resolve_fallback(raw.truncate_lines, base, category, |t| t.truncate_lines),
        truncate_bytes: resolve_fallback(raw.truncate_bytes, base, category, |t| t.truncate_bytes),
        timeout_seconds: resolve_fallback(raw.timeout_seconds, base, category, |t| {
            t.timeout_seconds
        })
        .unwrap_or(15),
        error_fallback: resolve_fallback(raw.error_fallback.clone(), base, category, |t| {
            t.error_fallback.clone()
        }),
        empty_fallback: resolve_fallback(raw.empty_fallback.clone(), base, category, |t| {
            t.empty_fallback.clone()
        }),
        success_exit_codes: resolve_fallback(raw.success_exit_codes.clone(), base, category, |t| {
            t.success_exit_codes.clone()
        })
        .unwrap_or_default(),
    }
}

/// Merges a raw tool definition with its optional base definition.
#[must_use]
pub fn merge_tool(name: String, base: Option<&RawTool>, raw: &RawTool) -> ToolDefinition {
    merge_tool_with_category(name, None, base, raw)
}

/// Resolves inheritance for all tools defined in `RawToolsConfig`, merging category defaults,
/// base tool templates, and concrete tool declarations.
#[must_use]
pub fn resolve(config: &RawToolsConfig) -> Vec<ToolDefinition> {
    let mut resolved = Vec::with_capacity(config.tools.len());
    for (name, raw_tool) in &config.tools {
        let base = raw_tool.inherit.as_deref().and_then(|key| {
            let clean = key.strip_prefix("base_tools.").unwrap_or(key);
            config.base_tools.get(clean)
        });
        let category_name = raw_tool
            .category
            .as_deref()
            .or_else(|| base?.category.as_deref());
        let category = category_name.and_then(|cat| config.categories.get(cat));
        resolved.push(merge_tool_with_category(
            name.clone(),
            category,
            base,
            raw_tool,
        ));
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
        assert_eq!(merged.required_gates(), <&[String]>::default());
    }

    #[test]
    fn test_merge_category_inheritance_and_gates() {
        let mut categories = BTreeMap::new();
        categories.insert(
            "compilation".to_owned(),
            RawTool {
                tracks_circuit_breaker: Some(true),
                require_gates: Some(vec!["vcs_conflicts".to_owned()]),
                timeout_seconds: Some(60),
                ..Default::default()
            },
        );

        let mut tools = BTreeMap::new();
        // Tool 1: inherits category gates and adds extra gate
        tools.insert(
            "cargo_clippy".to_owned(),
            RawTool {
                category: Some("compilation".to_owned()),
                require_gates: Some(vec!["compilation".to_owned()]),
                ..Default::default()
            },
        );
        // Tool 2: inherits category gates but opts out via skip_gates
        tools.insert(
            "run_special_check".to_owned(),
            RawTool {
                category: Some("compilation".to_owned()),
                skip_gates: Some(vec!["vcs_conflicts".to_owned()]),
                ..Default::default()
            },
        );

        let config = RawToolsConfig {
            base_tools: BTreeMap::new(),
            categories,
            tools,
        };

        let resolved = resolve(&config);
        assert_eq!(resolved.len(), 2);

        let clippy = resolved.iter().find(|t| t.name == "cargo_clippy").unwrap();
        assert!(clippy.tracks_circuit_breaker());
        assert_eq!(clippy.timeout_seconds, 60);
        // Has both category gate and tool-added gate
        assert_eq!(
            clippy.preconditions.require_gates,
            vec!["vcs_conflicts".to_owned(), "compilation".to_owned()]
        );

        let special = resolved
            .iter()
            .find(|t| t.name == "run_special_check")
            .unwrap();
        assert!(special.tracks_circuit_breaker());
        // vcs_conflicts was skipped!
        assert_eq!(special.preconditions.require_gates, Vec::<String>::new());
    }
}
