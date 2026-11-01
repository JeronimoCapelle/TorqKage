//! `cli_manual` action handler providing in-session help and manual lookups.

use std::process::Command;
use crate::guards::truncate_output;
use crate::models::ToolDefinition;

/// Executes a CLI manual lookup action (`query_tool_help`).
///
/// Returns a tuple of `(output_text, is_error)`.
pub(crate) fn execute(
    tool: &ToolDefinition,
    arguments: &serde_json::Value,
) -> (String, bool) {
    let tool_name = arguments
        .get("tool_name")
        .and_then(serde_json::Value::as_str)
        .map(str::trim);

    let content = match tool_name {
        Some(name) if !name.is_empty() => query_tool_help(name),
        _ => default_tool_catalog(),
    };

    let truncated = truncate_output(
        &content,
        tool.truncate_lines.or(Some(100)),
        tool.truncate_bytes,
        tool.emit_truncation_notice,
    );

    (truncated, false)
}

fn query_tool_help(name: &str) -> String {
    // 1. Try `--help`
    let output = Command::new(name).arg("--help").output();

    if let Ok(out) = output
        && (out.status.success() || !out.stdout.is_empty())
    {
        let stdout_str = String::from_utf8_lossy(&out.stdout);
        if !stdout_str.trim().is_empty() {
            return stdout_str.into_owned();
        }
    }

    // 2. Try `tldr`
    let tldr_output = Command::new("tldr").arg(name).output();
    if let Ok(out) = tldr_output
        && out.status.success()
    {
        let tldr_str = String::from_utf8_lossy(&out.stdout);
        if !tldr_str.trim().is_empty() {
            return tldr_str.into_owned();
        }
    }

    format!("No manual or --help usage output available for command '{name}' on host system.")
}

fn default_tool_catalog() -> String {
    "# Available CLI Tooling in agy-suite\n\
     - cargo-nextest: Parallel test harness runner\n\
     - cargo-mutants: Mutation testing framework\n\
     - cargo-insta: Snapshot testing review tool\n\
     - cargo-deny: Supply-chain security, RustSec advisories, licenses\n\
     - cargo-machete: Unused Cargo.toml dependency scanner\n\
     - cargo-llvm-cov: LLVM source-based code coverage\n\
     - cargo-bloat: Binary size and monomorphization analyzer\n\
     - cargo-expand: Macro expansion introspection\n\
     - cargo-modules: Module tree visualization\n\
     - agy-hooks: Ephemeral Antigravity lifecycle hook runner\n"
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
        assert!(out.contains("Available CLI Tooling"));
    }
}
