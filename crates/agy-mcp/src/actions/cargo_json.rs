//! `cargo_json_diagnostics` action handler parsing compiler JSON streams.

use std::{path::Path, process::Command};

use agy_common::SubprocessSupervisor;

use crate::{guards::truncate_output, models::ToolDefinition};

/// Executes a cargo command with `--message-format=json`, filtering compiler diagnostics.
///
/// Returns a tuple of `(output_text, is_error)`.
///
/// # Errors
/// Returns an error message if the cargo process cannot be spawned.
pub(crate) fn execute(
    tool: &ToolDefinition,
    workspace_root: &Path,
) -> Result<(String, bool), String> {
    if tool.command.is_empty() {
        return Err(format!("Tool '{}' has no command configured", tool.name));
    }

    let binary = tool
        .command
        .first()
        .ok_or_else(|| "Empty command".to_owned())?;
    if !binary.contains('/') && which::which(binary).is_err() {
        return Err(format!(
            "Configured compiler binary '{binary}' for tool '{}' is not installed on host ($PATH)",
            tool.name
        ));
    }
    let mut args: Vec<String> = tool.command.get(1..).unwrap_or(&[]).to_vec();

    // Ensure --color=never is supplied if not already present
    if !args
        .iter()
        .any(|a| a == "--color" || a.starts_with("--color="))
    {
        args.push("--color=never".to_owned());
    }

    let mut cmd = Command::new(binary);
    cmd.args(&args);
    cmd.current_dir(workspace_root);

    let output = SubprocessSupervisor::run_supervised(cmd, tool.timeout_seconds)?;

    let stdout_str = String::from_utf8_lossy(&output.stdout);
    let (rendered_diagnostics, has_errors) = parse_cargo_json(&stdout_str);

    if rendered_diagnostics.is_empty() {
        if output.status.success() {
            return Ok((
                "Verification passed: 0 errors, 0 warnings.".to_owned(),
                false,
            ));
        }

        // Check if stderr contains something relevant (e.g. build failure or panic)
        let stderr_str = String::from_utf8_lossy(&output.stderr);
        let trimmed_stderr = stderr_str.trim();
        if !trimmed_stderr.is_empty() {
            return Ok((
                format!("Verification check failed:\n{trimmed_stderr}"),
                true,
            ));
        }

        return Ok((
            "Verification failed with non-zero exit code.".to_owned(),
            true,
        ));
    }

    let joined = rendered_diagnostics.join("\n\n");
    let truncated = truncate_output(
        &joined,
        tool.truncate_lines.or(Some(120)),
        tool.truncate_bytes,
        tool.emit_truncation_notice(),
    );

    Ok((truncated, has_errors || !output.status.success()))
}

fn parse_cargo_json(stdout: &str) -> (Vec<String>, bool) {
    let mut diagnostics = Vec::new();
    let mut has_errors = false;

    for line in stdout.lines() {
        let trimmed = line.trim();
        if !trimmed.starts_with('{') {
            continue;
        }

        if let Ok(val) = serde_json::from_str::<serde_json::Value>(trimmed)
            && val.get("reason").and_then(serde_json::Value::as_str) == Some("compiler-message")
            && let Some(message) = val.get("message")
        {
            let level = message
                .get("level")
                .and_then(serde_json::Value::as_str)
                .unwrap_or("info");
            if level == "error" {
                has_errors = true;
            }

            if let Some(rendered) = message.get("rendered").and_then(serde_json::Value::as_str) {
                let clean = strip_ansi_escapes(rendered);
                if !clean.trim().is_empty() {
                    diagnostics.push(clean);
                }
            }
        }
    }

    (diagnostics, has_errors)
}

fn strip_ansi_escapes(input: &str) -> String {
    let mut out = String::with_capacity(input.len());
    let mut in_escape = false;

    for ch in input.chars() {
        if ch == '\x1b' {
            in_escape = true;
        } else if in_escape {
            if ch == 'm' {
                in_escape = false;
            }
        } else {
            out.push(ch);
        }
    }

    out
}

#[cfg(test)]
#[allow(
    clippy::expect_used,
    clippy::unwrap_used,
    clippy::panic,
    clippy::indexing_slicing,
    clippy::assertions_on_result_states,
    reason = "Unit test assertions"
)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_cargo_json_compiler_message() {
        let json_line = r#"{"reason":"compiler-message","message":{"rendered":"error[E0308]: mismatched types\n --> src/main.rs:2:5","code":{"code":"E0308"},"level":"error"}}"#;
        let (diags, has_err) = parse_cargo_json(json_line);

        assert!(has_err);
        assert_eq!(diags.len(), 1);
        assert!(diags[0].contains("E0308"));
    }

    #[test]
    fn test_strip_ansi_escapes() {
        let ansi = "\x1b[31merror\x1b[0m: broken code";
        assert_eq!(strip_ansi_escapes(ansi), "error: broken code");
    }
}
