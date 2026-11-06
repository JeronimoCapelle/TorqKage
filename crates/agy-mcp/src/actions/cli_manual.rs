//! `cli_manual` action handler providing in-session help, tldr, navi recipes, and man snippets.

use std::process::Command;
use agy_common::{
    DEFAULT_HELP_PREVIEW_LINES, DEFAULT_MAN_PREVIEW_LINES, DEFAULT_QUICK_COMMAND_TIMEOUT_SECS,
    SubprocessSupervisor,
};
use crate::guards::truncate_output;
use crate::models::ToolDefinition;

/// Executes a CLI manual lookup action (`query_tool_help`).
///
/// Priority ladder:
/// 1. `navi` cheatsheet recipe (if `query` parameter provided)
/// 2. `tldr` cheatsheet (concise real-world examples)
/// 3. `--help` CLI output (short flag reference, capped at 60 lines)
/// 4. `man` synopsis snippet (NAME + SYNOPSIS block for Linux system binaries)
/// 5. Default workspace tool catalog
pub(crate) fn execute(
    tool: &ToolDefinition,
    arguments: &serde_json::Value,
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
        Some(name) if !name.is_empty() => resolve_tool_help(name, query),
        _ => default_tool_catalog(),
    };

    let truncated = truncate_output(
        &content,
        tool.truncate_lines.or(Some(80)),
        tool.truncate_bytes,
        tool.emit_truncation_notice(),
    );

    (truncated, false)
}

fn resolve_tool_help(name: &str, query_opt: Option<&str>) -> String {
    let tool_installed = which::which(name).is_ok();
    let navi_installed = which::which("navi").is_ok();
    let tldr_installed = which::which("tldr").is_ok();
    let man_installed = which::which("man").is_ok();

    // 1. If a specific query/topic was provided, check navi first (if installed)
    if navi_installed
        && let Some(query) = query_opt
        && !query.is_empty()
        && let Some(recipe) = query_navi(name, query)
    {
        return format!("# navi cheatsheet recipe: {name} ({query})\n\n```bash\n{recipe}\n```");
    }

    // 2. Query tldr for concise, task-oriented examples (if installed)
    if tldr_installed
        && let Some(tldr) = query_tldr(name)
    {
        return format!("# tldr cheatsheet: {name}\n\n{tldr}");
    }

    // 3. Query navi with tool name alone if tldr was absent (if installed)
    if navi_installed
        && let Some(recipe) = query_navi(name, "")
    {
        return format!("# navi cheatsheet recipe: {name}\n\n```bash\n{recipe}\n```");
    }

    // 4. Query tool's native `--help` flag (only if tool binary is actually installed)
    if tool_installed
        && let Some(help) = query_cli_help(name)
    {
        return format!("# {name} --help\n\n{help}");
    }

    // 5. Query man page and extract concise NAME + SYNOPSIS block (if installed)
    if man_installed
        && let Some(man_synopsis) = query_man_synopsis(name)
    {
        return format!("# man synopsis: {name}\n\n{man_synopsis}");
    }

    if !tool_installed {
        return format!(
            "Tool '{name}' is not installed on the host system ($PATH), and no local tldr/navi/man documentation was found."
        );
    }

    let mut msg = format!("No manual, tldr, navi recipe, or --help usage output available for command '{name}' on host system.");
    if !tldr_installed && !navi_installed {
        msg.push_str("\n\n[Advisory]: Cheatsheet utilities 'tldr' and 'navi' are not installed on this host. Installing them enables rich offline command recipes.");
    }
    msg
}

fn query_navi(name: &str, query: &str) -> Option<String> {
    let search_term = if query.is_empty() {
        name.to_owned()
    } else {
        format!("{name} {query}")
    };

    let mut cmd = Command::new("navi");
    cmd.args(["--query", &search_term, "--best-match", "--print"]);
    let output = SubprocessSupervisor::run_supervised(cmd, DEFAULT_QUICK_COMMAND_TIMEOUT_SECS).ok()?;

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
    cmd.arg(name);
    let output = SubprocessSupervisor::run_supervised(cmd, DEFAULT_QUICK_COMMAND_TIMEOUT_SECS).ok()?;

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
    let output = SubprocessSupervisor::run_supervised(cmd, DEFAULT_QUICK_COMMAND_TIMEOUT_SECS).ok()?;

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
    cmd.arg(name);
    let output = SubprocessSupervisor::run_supervised(cmd, DEFAULT_QUICK_COMMAND_TIMEOUT_SECS).ok()?;

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

fn default_tool_catalog() -> String {
    "# CLI Tool Manual & Reference Guide\n\
     \n\
     Specify a tool name using the `tool_name` parameter to inspect manuals, usage cheatsheets, and recipes.\n\
     \n\
     Examples:\n\
     - `query_tool_help(tool_name=\"git\")`: View command synopsis and help.\n\
     - `query_tool_help(tool_name=\"git\", query=\"rebase\")`: View recipes for git rebase.\n"
        .to_owned()
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
        let (out, is_err) = execute(&tool, &args);
        assert!(!is_err);
        assert!(out.contains("CLI Tool Manual"));
    }

    #[test]
    fn test_extract_man_snippet() {
        let sample = "PERF(1) Manual\n\nNAME\n    perf - analysis\n\nSYNOPSIS\n    perf [OPTIONS]\n\nOPTIONS\n    -h, --help";
        let snippet = extract_man_snippet(sample).expect("snippet extraction failed");
        assert!(snippet.contains("NAME"));
        assert!(snippet.contains("SYNOPSIS"));
        assert!(!snippet.contains("-h, --help"));
    }
}
