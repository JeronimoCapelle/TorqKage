//! `cargo_json_diagnostics` action handler parsing compiler JSON streams and attaching rustc explanations.

use std::collections::HashSet;
use std::path::Path;
use std::process::Command;
use crate::guards::truncate_output;
use crate::models::ToolDefinition;

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

    let binary = tool.command.first().ok_or_else(|| "Empty command".to_owned())?;
    let mut args: Vec<String> = tool.command.get(1..).unwrap_or(&[]).to_vec();

    // Ensure --color=never is supplied if not already present
    if !args.iter().any(|a| a == "--color" || a.starts_with("--color=")) {
        args.push("--color=never".to_owned());
    }

    let output = Command::new(binary)
        .args(&args)
        .current_dir(workspace_root)
        .output()
        .map_err(|err| format!("Failed to spawn cargo command '{binary}': {err}"))?;

    let stdout_str = String::from_utf8_lossy(&output.stdout);
    let (rendered_diagnostics, error_codes, has_errors) = parse_cargo_json(&stdout_str);

    let mut result_parts = Vec::new();

    if rendered_diagnostics.is_empty() {
        if output.status.success() {
            return Ok(("Cargo verification passed: 0 errors, 0 warnings.".to_owned(), false));
        }

        // Check if stderr contains something relevant (e.g. build failure or panic)
        let stderr_str = String::from_utf8_lossy(&output.stderr);
        let trimmed_stderr = stderr_str.trim();
        if !trimmed_stderr.is_empty() {
            return Ok((format!("Cargo check failed:\n{trimmed_stderr}"), true));
        }

        return Ok(("Cargo verification failed with non-zero exit code.".to_owned(), true));
    }

    result_parts.push(rendered_diagnostics.join("\n\n"));

    // Attach rustc explanation for failing error codes (E0xxx) if enabled
    if tool.attach_rustc_explain && !error_codes.is_empty() {
        let mut explanations = Vec::new();
        for code in &error_codes {
            if code.starts_with('E') && code.len() == 5
                && let Some(explain_snippet) = fetch_rustc_explanation(code)
            {
                explanations.push(format!("### Explanation for {code}\n{explain_snippet}"));
            }
        }

        if !explanations.is_empty() {
            result_parts.push("\n---\n".to_owned());
            result_parts.push(explanations.join("\n\n"));
        }
    }

    let joined = result_parts.join("\n");
    let truncated = truncate_output(
        &joined,
        tool.truncate_lines.or(Some(120)),
        tool.truncate_bytes,
        tool.emit_truncation_notice,
    );

    Ok((truncated, has_errors || !output.status.success()))
}

fn parse_cargo_json(stdout: &str) -> (Vec<String>, HashSet<String>, bool) {
    let mut diagnostics = Vec::new();
    let mut error_codes = HashSet::new();
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
            let level = message.get("level").and_then(serde_json::Value::as_str).unwrap_or("info");
            if level == "error" {
                has_errors = true;
            }

            if let Some(code_val) = message.get("code").and_then(|c| c.get("code")).and_then(serde_json::Value::as_str) {
                error_codes.insert(code_val.to_owned());
            }

            if let Some(rendered) = message.get("rendered").and_then(serde_json::Value::as_str) {
                let clean = strip_ansi_escapes(rendered);
                if !clean.trim().is_empty() {
                    diagnostics.push(clean);
                }
            }
        }
    }

    (diagnostics, error_codes, has_errors)
}

fn fetch_rustc_explanation(code: &str) -> Option<String> {
    let output = Command::new("rustc")
        .args(["--explain", code])
        .output()
        .ok()?;

    if output.status.success() {
        let text = String::from_utf8_lossy(&output.stdout);
        // Take first 15 lines of explanation
        let lines: Vec<&str> = text.lines().take(15).collect();
        if !lines.is_empty() {
            return Some(lines.join("\n"));
        }
    }
    None
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
        let (diags, codes, has_err) = parse_cargo_json(json_line);

        assert!(has_err);
        assert_eq!(diags.len(), 1);
        assert!(diags[0].contains("E0308"));
        assert!(codes.contains("E0308"));
    }

    #[test]
    fn test_strip_ansi_escapes() {
        let ansi = "\x1b[31merror\x1b[0m: broken code";
        assert_eq!(strip_ansi_escapes(ansi), "error: broken code");
    }
}
