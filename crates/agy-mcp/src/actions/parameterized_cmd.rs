//! `parameterized_cmd` action handler executing bounded host binaries with argument regex validation.

use std::path::Path;
use std::process::Command;
use crate::guards::truncate_output;
use crate::models::{OutputCapture, ToolDefinition};

/// Executes a parameterized CLI command with argument validation.
///
/// Returns a tuple of `(output_text, is_error)`.
///
/// # Errors
/// Returns an error message if argument validation fails or the process fails to spawn.
pub(crate) fn execute(
    tool: &ToolDefinition,
    workspace_root: &Path,
    arguments: &serde_json::Value,
) -> Result<(String, bool), String> {
    if tool.command.is_empty() {
        return Err(format!("Tool '{}' has no command configured", tool.name));
    }

    let mut dynamic_args = Vec::new();

    for arg in &tool.args {
        let val_opt = arguments
            .get(&arg.name)
            .and_then(serde_json::Value::as_str)
            .map(str::trim);

        if let Some(val) = val_opt
            && !val.is_empty()
        {
            if let Some(pattern) = &arg.regex
                && !matches_pattern(val, pattern)
            {
                return Err(format!(
                    "Security validation error: Argument '{}' with value '{val}' violates pattern '{pattern}'",
                    arg.name
                ));
            }

            if let Some(flag) = &arg.flag {
                dynamic_args.push(flag.clone());
            }
            dynamic_args.push(val.to_owned());
        } else if arg.required {
            return Err(format!(
                "Missing required argument '{}' for tool '{}'",
                arg.name, tool.name
            ));
        }
    }

    let binary = tool.command.first().ok_or_else(|| "Empty command".to_owned())?;
    let base_args = tool.command.get(1..).unwrap_or(&[]);

    let mut cmd = Command::new(binary);
    cmd.args(base_args);
    cmd.args(&dynamic_args);
    cmd.current_dir(workspace_root);

    let output = cmd
        .output()
        .map_err(|err| format!("Failed to spawn command '{binary}': {err}"))?;

    let is_error = !output.status.success();

    let raw_text = match tool.output_capture {
        OutputCapture::ExitCodeOnly => {
            let code = output.status.code().unwrap_or(-1);
            format!("Process exited with status code {code}.")
        }

        OutputCapture::Stderr => String::from_utf8_lossy(&output.stderr).into_owned(),

        OutputCapture::Both => {
            let stdout_str = String::from_utf8_lossy(&output.stdout);
            let stderr_str = String::from_utf8_lossy(&output.stderr);
            if stderr_str.trim().is_empty() {
                stdout_str.into_owned()
            } else if stdout_str.trim().is_empty() {
                stderr_str.into_owned()
            } else {
                format!("{stdout_str}\n--- stderr ---\n{stderr_str}")
            }
        }

        OutputCapture::Stdout | OutputCapture::JsonSpans => {
            String::from_utf8_lossy(&output.stdout).into_owned()
        }
    };

    // Apply error fallback if configured and process failed
    let final_text = if is_error && tool.error_fallback.as_deref() == Some("token_smart_not_found") {
        if raw_text.contains("could not find") || raw_text.contains("not found") {
            format!(
                "Item not found in module tree. Verify item path or inspect module tree via inspect_module_structure.\n\n{raw_text}"
            )
        } else {
            raw_text
        }
    } else {
        raw_text
    };

    let truncated = truncate_output(
        &final_text,
        tool.truncate_lines,
        tool.truncate_bytes,
        tool.emit_truncation_notice,
    );

    Ok((truncated, is_error))
}

fn matches_pattern(val: &str, pattern: &str) -> bool {
    if val.is_empty() {
        return false;
    }

    if let Some(inner) = pattern.strip_prefix("^[").and_then(|p| p.strip_suffix("]+$")) {
        let has_alnum = inner.contains("a-zA-Z0-9")
            || (inner.contains("a-z") && inner.contains("A-Z") && inner.contains("0-9"));
        let has_colon = inner.contains(':');
        let has_underscore = inner.contains('_');
        let has_dash = inner.contains('-');

        return val.chars().all(|c| {
            (has_alnum && c.is_ascii_alphanumeric())
                || (has_underscore && c == '_')
                || (has_colon && c == ':')
                || (has_dash && c == '-')
        });
    }

    // Strict fallback: letters, numbers, underscore, colon, hyphen
    val.chars()
        .all(|c| c.is_ascii_alphanumeric() || c == '_' || c == ':' || c == '-')
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
    use crate::models::ToolArg;
    use serde_json::json;

    #[test]
    fn test_pattern_matching() {
        assert!(matches_pattern("Processor", "^[a-zA-Z0-9_:]+$"));
        assert!(matches_pattern("types::state", "^[a-zA-Z0-9_:]+$"));
        assert!(matches_pattern("unit-test_1", "^[a-zA-Z0-9_-]+$"));
        assert!(!matches_pattern("foo; rm -rf /", "^[a-zA-Z0-9_:]+$"));
        assert!(!matches_pattern("foo bar", "^[a-zA-Z0-9_:]+$"));
    }

    fn create_test_dir(tag: &str) -> std::path::PathBuf {
        let dir = std::env::temp_dir().join(format!(
            "agy-test-param-{tag}-{}",
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        std::fs::create_dir_all(&dir).expect("create test dir");
        dir
    }

    #[test]
    fn test_parameterized_cmd_echo() {
        let temp = create_test_dir("echo");
        let tool = ToolDefinition {
            name: "test_echo".to_owned(),
            command: vec!["echo".to_owned()],
            args: vec![
                ToolArg {
                    name: "msg".to_owned(),
                    required: true,
                    regex: Some("^[a-zA-Z0-9_-]+$".to_owned()),
                    flag: None,
                    description: "Message".to_owned(),
                },
                ToolArg {
                    name: "prefix".to_owned(),
                    required: false,
                    regex: Some("^[a-zA-Z0-9_-]+$".to_owned()),
                    flag: Some("-n".to_owned()),
                    description: "No newline".to_owned(),
                },
            ],
            output_capture: OutputCapture::Stdout,
            ..Default::default()
        };

        let args = json!({
            "msg": "hello_world"
        });

        let (out, is_err) = execute(&tool, &temp, &args).unwrap();
        assert!(!is_err);
        assert!(out.contains("hello_world"));
    }

    #[test]
    fn test_parameterized_cmd_injection_blocked() {
        let temp = create_test_dir("inject");
        let tool = ToolDefinition {
            name: "test_echo".to_owned(),
            command: vec!["echo".to_owned()],
            args: vec![ToolArg {
                name: "target".to_owned(),
                required: true,
                regex: Some("^[a-zA-Z0-9_-]+$".to_owned()),
                ..Default::default()
            }],
            ..Default::default()
        };

        let args = json!({
            "target": "target && cat /etc/passwd"
        });

        let res = execute(&tool, &temp, &args);
        assert!(res.is_err());
        assert!(res.unwrap_err().contains("violates pattern"));
    }
}
