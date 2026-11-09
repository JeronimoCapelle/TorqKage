//! `cli_manual` action handler providing in-session help, tldr, navi recipes, and man snippets.

use std::{fmt::Write, process::Command};

use agy_common::{
    DEFAULT_HELP_PREVIEW_LINES, DEFAULT_MAN_PREVIEW_LINES, DEFAULT_QUICK_COMMAND_TIMEOUT_SECS,
    SubprocessSupervisor,
};

use crate::{guards::truncate_output, models::ToolDefinition};

/// Executes a CLI manual lookup action (`query_tool_help`).
///
/// Priority ladder:
/// 1. agy-suite internal MCP tool definition & schema (if `tool_name` matches an MCP tool)
/// 2. `navi` cheatsheet recipe (if `query` parameter provided)
/// 3. `tldr` cheatsheet (concise real-world examples)
/// 4. `--help` CLI output (short flag reference, capped at 60 lines)
/// 5. `man` synopsis snippet (NAME + SYNOPSIS block for Linux system binaries)
/// 6. Default workspace tool catalog
pub(crate) fn execute(
    tool: &ToolDefinition,
    arguments: &serde_json::Value,
    available_tools: &[ToolDefinition],
) -> (String, bool) {
    let tool_name = arguments
        .get("tool_name")
        .and_then(serde_json::Value::as_str)
        .map(str::trim);

    let query = arguments
        .get("query")
        .and_then(serde_json::Value::as_str)
        .map(str::trim);

    let content = match tool_name {
        Some(name) if !name.is_empty() => resolve_tool_help(name, query, available_tools),
        _ => default_tool_catalog(tool, available_tools),
    };

    let truncated = truncate_output(
        &content,
        tool.truncate_lines.or(Some(80)),
        tool.truncate_bytes,
        tool.emit_truncation_notice(),
    );

    (truncated, false)
}

fn resolve_tool_help(
    name: &str,
    query_opt: Option<&str>,
    available_tools: &[ToolDefinition],
) -> String {
    // 0. Reject option/flag injection attempts
    if name.starts_with('-') {
        return format!(
            "Invalid tool name '{name}': tool names must start with an alphanumeric or underscore character."
        );
    }

    // 1. Check if `name` matches an internal agy-suite MCP tool
    if let Some(mcp_tool) = available_tools.iter().find(|t| t.name == name) {
        return format_mcp_tool_help(mcp_tool);
    }

    let tool_installed = which::which(name).is_ok();
    let navi_installed = which::which("navi").is_ok();
    let tldr_installed = which::which("tldr").is_ok();
    let man_installed = which::which("man").is_ok();

    // 2. If a specific query/topic was provided, check navi first (if installed)
    if navi_installed
        && let Some(query) = query_opt
        && !query.is_empty()
        && let Some(recipe) = query_navi(name, query)
    {
        return format!("# navi cheatsheet recipe: {name} ({query})\n\n```bash\n{recipe}\n```");
    }

    // 3. Query tldr for concise, task-oriented examples (if installed)
    if tldr_installed && let Some(tldr) = query_tldr(name) {
        return format!("# tldr cheatsheet: {name}\n\n{tldr}");
    }

    // 4. Query navi with tool name alone if tldr was absent (if installed)
    if navi_installed && let Some(recipe) = query_navi(name, "") {
        return format!("# navi cheatsheet recipe: {name}\n\n```bash\n{recipe}\n```");
    }

    // 5. Query tool's native `--help` flag (only if tool binary is actually installed)
    if tool_installed && let Some(help) = query_cli_help(name) {
        return format!("# {name} --help\n\n{help}");
    }

    // 6. Query man page and extract concise NAME + SYNOPSIS block (if installed)
    if man_installed && let Some(man_synopsis) = query_man_synopsis(name) {
        return format!("# man synopsis: {name}\n\n{man_synopsis}");
    }

    if !tool_installed {
        return format!(
            "Tool '{name}' is neither a recognized agy-suite MCP tool nor a host CLI binary on $PATH."
        );
    }

    let mut msg = format!(
        "No manual, tldr, navi recipe, or --help usage output available for command '{name}' on host system."
    );
    if !tldr_installed && !navi_installed {
        msg.push_str("\n\n[Advisory]: Cheatsheet utilities 'tldr' and 'navi' are not installed on this host. Installing them enables rich offline command recipes.");
    }
    msg
}

fn format_mcp_tool_help(tool: &ToolDefinition) -> String {
    let mut out = String::new();
    _ = writeln!(out, "# agy-suite MCP Tool: `{}`", tool.name);
    _ = writeln!(out, "\n**Description:** {}", tool.description);
    if let Some(cat) = &tool.category {
        _ = writeln!(out, "**Category:** {cat}");
    }
    _ = writeln!(out, "**Action Handler:** {}", action_name(&tool.action));

    if !tool.allowed_directories.is_empty() {
        _ = writeln!(
            out,
            "**Allowed Directories:** {}",
            tool.allowed_directories.join(", ")
        );
    }
    if let Some(dir) = &tool.enforce_dir {
        _ = writeln!(out, "**Enforced Directory:** {dir}");
    }

    _ = writeln!(out, "\n### Parameters & Arguments:");
    format_tool_parameters(&mut out, tool);

    _ = writeln!(
        out,
        "\n> [!NOTE]\n> This is an agy-suite MCP tool. Invoke it via `call_mcp_tool` using `ServerName: \"agy-tools\"` and `ToolName: \"{}\"` (or as a native tool if eagerly loaded).",
        tool.name
    );

    out
}

const fn action_name(action: &crate::models::ToolAction) -> &'static str {
    match action {
        crate::models::ToolAction::FileRead => "file_read",
        crate::models::ToolAction::FileWrite => "file_write",
        crate::models::ToolAction::MpatchEdit => "mpatch_edit",
        crate::models::ToolAction::FileSearch => "file_search",
        crate::models::ToolAction::StaticCommand => "static_command",
        crate::models::ToolAction::ParameterizedCommand => "parameterized_command",
        crate::models::ToolAction::CliManual => "cli_manual",
        crate::models::ToolAction::ProjectState => "project_state",
        crate::models::ToolAction::WorkspaceEnsure => "workspace_ensure",
        crate::models::ToolAction::WorkspaceInspect => "workspace_inspect",
        crate::models::ToolAction::WorkspaceMerge => "workspace_merge",
        crate::models::ToolAction::SetActiveAgent => "set_active_agent",
        crate::models::ToolAction::RecordSessionHandoff => "record_session_handoff",
        crate::models::ToolAction::DetectWip => "detect_wip",
        crate::models::ToolAction::Unknown(_) => "custom",
    }
}

fn format_tool_parameters(out: &mut String, tool: &ToolDefinition) {
    let schema = crate::schema::McpTool::from_definition(tool).input_schema;
    let required_set: Vec<&str> = schema
        .get("required")
        .and_then(serde_json::Value::as_array)
        .map_or_else(Vec::new, |arr| {
            arr.iter().filter_map(serde_json::Value::as_str).collect()
        });

    if let Some(props) = schema
        .get("properties")
        .and_then(serde_json::Value::as_object)
    {
        if props.is_empty() {
            out.push_str("- (No parameters required)\n");
        } else {
            for (param_name, param_val) in props {
                let param_type = param_val
                    .get("type")
                    .and_then(serde_json::Value::as_str)
                    .unwrap_or("string");
                let param_desc = param_val
                    .get("description")
                    .and_then(serde_json::Value::as_str)
                    .unwrap_or("");
                let req_label = if required_set.contains(&param_name.as_str()) {
                    " (required)"
                } else {
                    " (optional)"
                };
                if param_desc.is_empty() {
                    _ = writeln!(out, "- `{param_name}` ({param_type}){req_label}");
                } else {
                    _ = writeln!(
                        out,
                        "- `{param_name}` ({param_type}){req_label}: {param_desc}"
                    );
                }
            }
        }
    } else {
        out.push_str("- (No parameters required)\n");
    }
}

fn query_navi(name: &str, query: &str) -> Option<String> {
    let search_term = if query.is_empty() {
        name.to_owned()
    } else {
        format!("{name} {query}")
    };

    let mut cmd = Command::new("navi");
    cmd.args(["--query", &search_term, "--best-match", "--print"]);
    let output =
        SubprocessSupervisor::run_supervised(cmd, DEFAULT_QUICK_COMMAND_TIMEOUT_SECS).ok()?;

    if output.status.success() {
        let text = String::from_utf8_lossy(&output.stdout).trim().to_owned();
        if !text.is_empty() {
            return Some(text);
        }
    }
    None
}

fn query_tldr(name: &str) -> Option<String> {
    let mut cmd = Command::new("tldr");
    cmd.args(["--", name]);
    let output =
        SubprocessSupervisor::run_supervised(cmd, DEFAULT_QUICK_COMMAND_TIMEOUT_SECS).ok()?;

    if output.status.success() {
        let text = String::from_utf8_lossy(&output.stdout).trim().to_owned();
        if !text.is_empty() && !text.contains("not found in cache") {
            return Some(text);
        }
    }
    None
}

fn query_cli_help(name: &str) -> Option<String> {
    let mut cmd = Command::new(name);
    cmd.arg("--help");
    let output =
        SubprocessSupervisor::run_supervised(cmd, DEFAULT_QUICK_COMMAND_TIMEOUT_SECS).ok()?;

    if output.status.success() || !output.stdout.is_empty() {
        let text = String::from_utf8_lossy(&output.stdout).trim().to_owned();
        if !text.is_empty() {
            // Cap raw --help preview to keep token budget frugal
            let sample: Vec<&str> = text.lines().take(DEFAULT_HELP_PREVIEW_LINES).collect();
            return Some(sample.join("\n"));
        }
    }
    None
}

fn query_man_synopsis(name: &str) -> Option<String> {
    let mut cmd = Command::new("man");
    cmd.env("MANPAGER", "cat");
    cmd.args(["--", name]);
    let output =
        SubprocessSupervisor::run_supervised(cmd, DEFAULT_QUICK_COMMAND_TIMEOUT_SECS).ok()?;

    if output.status.success() {
        let text = String::from_utf8_lossy(&output.stdout);
        return extract_man_snippet(&text);
    }
    None
}

fn extract_man_snippet(man_text: &str) -> Option<String> {
    let mut snippet_lines = Vec::new();
    let mut count: usize = 0;
    let mut started = false;

    for line in man_text.lines() {
        let trimmed = line.trim();

        // Stop if we hit the exhaustive OPTIONS block or exceed preview lines limit
        if started && (trimmed == "OPTIONS" || count >= DEFAULT_MAN_PREVIEW_LINES) {
            break;
        }

        if started {
            snippet_lines.push(line);
            count = count.saturating_add(1);
        } else if !trimmed.is_empty() {
            started = true;
            snippet_lines.push(line);
            count = count.saturating_add(1);
        }
    }

    if snippet_lines.is_empty() {
        None
    } else {
        Some(snippet_lines.join("\n"))
    }
}

fn default_tool_catalog(tool: &ToolDefinition, available_tools: &[ToolDefinition]) -> String {
    let mut out = String::new();
    out.push_str("# Tool Catalog & Command Reference\n\n");
    _ = writeln!(
        out,
        "Specify a tool name via `{}(tool_name = \"...\")` to inspect live tool definitions or host CLI manuals.\n",
        tool.name
    );

    if !available_tools.is_empty() {
        out.push_str("## Available MCP Tools:\n");
        for t in available_tools {
            let cat = t.category.as_deref().unwrap_or("general");
            _ = writeln!(out, "- `{}` ({}): {}", t.name, cat, t.description);
        }
        out.push('\n');
    }

    out.push_str("## Host CLI Utilities ($PATH):\n");
    _ = writeln!(
        out,
        "Pass any installed host CLI binary to inspect its live synopsis, `--help`, and usage recipes:\n\
         - `{}(tool_name = \"<binary>\")`\n\
         - `{}(tool_name = \"<binary>\", query = \"<topic>\")`\n",
        tool.name, tool.name
    );

    out
}

#[cfg(test)]
#[allow(
    clippy::expect_used,
    clippy::unwrap_used,
    clippy::panic,
    reason = "Unit test assertions"
)]
mod tests {
    use super::*;

    #[test]
    fn test_default_catalog_returned_when_empty() {
        let tool = ToolDefinition::default();
        let args = serde_json::json!({});
        let (out, is_err) = execute(&tool, &args, &[]);
        assert!(!is_err);
        assert!(out.contains("Tool Catalog & Command Reference"));
    }

    #[test]
    fn test_query_mcp_tool_help() {
        let tool = ToolDefinition::default();
        let mcp_tools = vec![ToolDefinition {
            name: "custom_diagnostic_tool".to_owned(),
            description: "Inspects system health".to_owned(),
            category: Some("diagnostics".to_owned()),
            ..Default::default()
        }];
        let args = serde_json::json!({ "tool_name": "custom_diagnostic_tool" });
        let (out, is_err) = execute(&tool, &args, &mcp_tools);
        assert!(!is_err);
        assert!(out.contains("agy-suite MCP Tool: `custom_diagnostic_tool`"));
        assert!(out.contains("Inspects system health"));
    }

    #[test]
    fn test_query_unknown_tool_guidance() {
        let tool = ToolDefinition::default();
        let args = serde_json::json!({ "tool_name": "definitely_nonexistent_tool_xyz_123" });
        let (out, is_err) = execute(&tool, &args, &[]);
        assert!(!is_err);
        assert!(out.contains("neither a recognized agy-suite MCP tool nor a host CLI binary"));
    }

    #[test]
    fn test_extract_man_snippet() {
        let sample = "PERF(1) Manual\n\nNAME\n    perf - analysis\n\nSYNOPSIS\n    perf [OPTIONS]\n\nOPTIONS\n    -h, --help";
        let snippet = extract_man_snippet(sample).expect("snippet extraction failed");
        assert!(snippet.contains("NAME"));
        assert!(snippet.contains("SYNOPSIS"));
        assert!(!snippet.contains("-h, --help"));
    }

    #[test]
    fn test_query_tool_help_dash_rejected() {
        let tool = ToolDefinition::default();
        let args = serde_json::json!({ "tool_name": "--help" });
        let (out, is_err) = execute(&tool, &args, &[]);
        assert!(!is_err);
        assert!(out.contains("Invalid tool name '--help'"));
    }
}
