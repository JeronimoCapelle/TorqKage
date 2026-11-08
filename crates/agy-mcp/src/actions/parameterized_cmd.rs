//! `parameterized_cmd` action handler executing bounded host binaries with argument regex validation.

use std::path::Path;

use crate::{
    actions::execute_supervised_command_with_detector,
    guards::truncate_output,
    models::{OutputCapture, ToolDefinition},
};

fn validate_and_build_args(
    tool: &ToolDefinition,
    arguments: &serde_json::Value,
) -> Result<Vec<String>, String> {
    let mut dynamic_args = Vec::new();

    for arg in &tool.args {
        if arg.switch {
            let is_active = match arguments.get(&arg.name) {
                Some(serde_json::Value::Bool(b)) => *b,
                Some(serde_json::Value::String(s)) => {
                    let trimmed = s.trim().to_ascii_lowercase();
                    trimmed == "true" || trimmed == "1" || trimmed == "yes"
                },
                _ => arg
                    .default
                    .as_deref()
                    .map(|d| d.trim().to_ascii_lowercase())
                    .is_some_and(|d| d == "true" || d == "1" || d == "yes"),
            };

            if is_active && let Some(flag) = &arg.flag {
                dynamic_args.push(flag.clone());
            }
            continue;
        }

        let val_opt = arguments.get(&arg.name).and_then(|v| match v {
            serde_json::Value::String(s) => Some(s.trim().to_owned()),
            serde_json::Value::Number(n) => Some(n.to_string()),
            _ => None,
        });

        let effective_val = match val_opt {
            Some(v) if !v.is_empty() => Some(v),
            _ => arg.default.clone(),
        };

        if let Some(val) = effective_val {
            if let Some(pattern) = &arg.regex
                && !matches_pattern(&val, pattern)
            {
                return Err(format!(
                    "Security validation error: Argument '{}' with value '{val}' violates pattern '{pattern}'",
                    arg.name
                ));
            }

            if let Some(flag) = &arg.flag {
                dynamic_args.push(flag.clone());
            }
            dynamic_args.push(val);
        } else if arg.required {
            return Err(format!(
                "Missing required argument '{}' for tool '{}'",
                arg.name, tool.name
            ));
        }
    }

    Ok(dynamic_args)
}

fn format_captured_output(output: &std::process::Output, tool: &ToolDefinition) -> String {
    match tool.output_capture {
        OutputCapture::ExitCodeOnly => {
            let code = output.status.code().unwrap_or(-1);
            format!("Process exited with status code {code}.")
        },

        OutputCapture::Stderr => {
            let stderr_str = String::from_utf8_lossy(&output.stderr);
            if stderr_str.trim().is_empty()
                && let Some(fallback) = &tool.empty_fallback
            {
                return fallback.clone();
            }
            stderr_str.into_owned()
        },

        OutputCapture::Both => {
            let stdout_str = String::from_utf8_lossy(&output.stdout);
            let stderr_str = String::from_utf8_lossy(&output.stderr);
            if stdout_str.trim().is_empty()
                && stderr_str.trim().is_empty()
                && let Some(fallback) = &tool.empty_fallback
            {
                return fallback.clone();
            }
            if stderr_str.trim().is_empty() {
                stdout_str.into_owned()
            } else if stdout_str.trim().is_empty() {
                stderr_str.into_owned()
            } else {
                format!("{stdout_str}\n--- stderr ---\n{stderr_str}")
            }
        },

        OutputCapture::Stdout | OutputCapture::JsonSpans => {
            let text = String::from_utf8_lossy(&output.stdout);
            if text.trim().is_empty() && !output.status.success() {
                let err = String::from_utf8_lossy(&output.stderr);
                if !err.trim().is_empty() {
                    return err.into_owned();
                }
            }
            if text.trim().is_empty()
                && let Some(fallback) = &tool.empty_fallback
            {
                return fallback.clone();
            }
            text.into_owned()
        },
    }
}

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
    execute_with_detector(
        tool,
        workspace_root,
        arguments,
        agy_common::sandbox::is_inside_container,
    )
}

/// Executes a parameterized CLI command with configurable sandbox detection.
pub(crate) fn execute_with_detector<F>(
    tool: &ToolDefinition,
    workspace_root: &Path,
    arguments: &serde_json::Value,
    is_outer_sandbox: F,
) -> Result<(String, bool), String>
where
    F: Fn() -> bool,
{
    if tool.command.is_empty() {
        return Err(format!("Tool '{}' has no command configured", tool.name));
    }

    let dynamic_args = validate_and_build_args(tool, arguments)?;

    let binary = tool
        .command
        .first()
        .ok_or_else(|| "Empty command".to_owned())?;
    let base_args = tool.command.get(1..).unwrap_or(&[]);

    let output = execute_supervised_command_with_detector(
        binary,
        (base_args, &dynamic_args),
        workspace_root,
        tool.timeout_seconds,
        tool.enforce_sandbox(),
        &tool.name,
        is_outer_sandbox,
    )?;

    let is_error = if let Some(code) = output.status.code() {
        if tool.success_exit_codes.contains(&code) {
            false
        } else {
            !output.status.success()
        }
    } else {
        !output.status.success()
    };

    let raw_text = format_captured_output(&output, tool);

    // Apply declarative error fallback if configured and process failed
    let final_text = if is_error
        && let Some(fallback) = tool.error_fallback.as_deref()
        && (raw_text.contains("could not find") || raw_text.contains("not found"))
    {
        let advice = if fallback == "token_smart_not_found" {
            "Item not found in module tree. Verify item path or inspect module hierarchy."
        } else {
            fallback
        };
        format!("{advice}\n\n{raw_text}")
    } else {
        raw_text
    };

    let truncated = truncate_output(
        &final_text,
        tool.truncate_lines,
        tool.truncate_bytes,
        tool.emit_truncation_notice(),
    );

    Ok((truncated, is_error))
}

fn matches_pattern(val: &str, pattern: &str) -> bool {
    if val.is_empty() {
        return false;
    }

    if let Ok(re) = regex::Regex::new(pattern) {
        return re.is_match(val);
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
    use serde_json::json;

    use super::*;
    use crate::models::ToolArg;

    #[test]
    fn test_pattern_matching() {
        assert!(matches_pattern("Processor", "^[a-zA-Z0-9_:]+$"));
        assert!(matches_pattern("types::state", "^[a-zA-Z0-9_:]+$"));
        assert!(matches_pattern("unit-test_1", "^[a-zA-Z0-9_-]+$"));
        assert!(matches_pattern(
            "struct ToolDefinition",
            "^[^\\x00-\\x1f]+$"
        ));
        assert!(!matches_pattern("bad\0text", "^[^\\x00-\\x1f]+$"));
        assert!(matches_pattern(
            "import sympy\nx = sympy.Symbol('x')",
            "^[^\\x00]+$"
        ));
        assert!(!matches_pattern("bad\0code", "^[^\\x00]+$"));
        assert!(!matches_pattern("foo; rm -rf /", "^[a-zA-Z0-9_:]+$"));
        assert!(!matches_pattern("foo bar", "^[a-zA-Z0-9_:]+$"));
        // Enhanced pattern tests: file paths, VCS revisions, rustc error codes
        assert!(matches_pattern(
            "crates/agy-mcp/src/lib.rs",
            "^[a-zA-Z0-9_./-]+$"
        ));
        assert!(matches_pattern("HEAD~1", "^[a-zA-Z0-9_@~^.:-]+$"));
        assert!(matches_pattern("default@", "^[a-zA-Z0-9_@~^.:-]+$"));
        assert!(matches_pattern("E0308", "^E[0-9]{4}$"));
        assert!(!matches_pattern("E308", "^E[0-9]{4}$"));
        assert!(!matches_pattern("E03088", "^E[0-9]{4}$"));
        assert!(!matches_pattern("F0308", "^E[0-9]{4}$"));
        // Standard regex features unlocked by the `regex` crate
        assert!(matches_pattern("release", "^(debug|release|test)$"));
        assert!(!matches_pattern("staging", "^(debug|release|test)$"));
        assert!(matches_pattern("12345", r"^\d{5}$"));
        assert!(!matches_pattern("1234", r"^\d{5}$"));
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
                    default: None,
                    ..Default::default()
                },
                ToolArg {
                    name: "prefix".to_owned(),
                    required: false,
                    regex: Some("^[a-zA-Z0-9_-]+$".to_owned()),
                    flag: Some("-n".to_owned()),
                    description: "No newline".to_owned(),
                    default: None,
                    ..Default::default()
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

    #[test]
    fn test_parameterized_cmd_ast_grep_no_matches() {
        if which::which("ast-grep").is_err() {
            return;
        }
        let temp = create_test_dir("ast_grep");
        let tool = ToolDefinition {
            name: "ast_grep_rewrite".to_owned(),
            command: vec![
                "ast-grep".to_owned(),
                "run".to_owned(),
                "--update-all".to_owned(),
            ],
            args: vec![
                ToolArg {
                    name: "pattern".to_owned(),
                    required: true,
                    flag: Some("-p".to_owned()),
                    regex: Some("^[^\\x00]+$".to_owned()),
                    description: "pattern".to_owned(),
                    default: None,
                    ..Default::default()
                },
                ToolArg {
                    name: "rewrite".to_owned(),
                    required: true,
                    flag: Some("-r".to_owned()),
                    regex: Some("^[^\\x00]+$".to_owned()),
                    description: "rewrite".to_owned(),
                    default: None,
                    ..Default::default()
                },
            ],
            output_capture: OutputCapture::Both,
            empty_fallback: Some("No matching AST patterns found to rewrite.".to_owned()),
            success_exit_codes: vec![0, 1],
            ..Default::default()
        };

        let args = json!({
            "pattern": "fn nonexistent_pattern_foo()",
            "rewrite": "fn replaced()"
        });

        let (out, is_err) = execute(&tool, &temp, &args).unwrap();
        assert!(!is_err);
        assert_eq!(out, "No matching AST patterns found to rewrite.");
    }

    #[test]
    fn test_parameterized_cmd_default_arg_used() {
        let temp = create_test_dir("default_arg");
        let tool = ToolDefinition {
            name: "test_default".to_owned(),
            command: vec!["echo".to_owned()],
            args: vec![
                ToolArg {
                    name: "msg".to_owned(),
                    required: true,
                    regex: Some("^[a-zA-Z0-9_-]+$".to_owned()),
                    flag: None,
                    description: "Message".to_owned(),
                    default: None,
                    ..Default::default()
                },
                ToolArg {
                    name: "path".to_owned(),
                    required: false,
                    regex: Some("^[a-zA-Z0-9_./-]+$".to_owned()),
                    flag: None,
                    description: "Path".to_owned(),
                    default: Some(".".to_owned()),
                    ..Default::default()
                },
            ],
            output_capture: OutputCapture::Stdout,
            ..Default::default()
        };

        let args = json!({
            "msg": "hello"
        });

        let (out, is_err) = execute(&tool, &temp, &args).unwrap();
        assert!(!is_err);
        assert!(out.contains("hello ."));
    }

    #[test]
    fn test_parameterized_cmd_switch_and_number_arg() {
        let temp = create_test_dir("switch_and_num");
        let tool = ToolDefinition {
            name: "test_switch".to_owned(),
            command: vec!["echo".to_owned()],
            args: vec![
                ToolArg {
                    name: "lib".to_owned(),
                    flag: Some("--lib".to_owned()),
                    switch: true,
                    ..Default::default()
                },
                ToolArg {
                    name: "depth".to_owned(),
                    flag: Some("--depth".to_owned()),
                    regex: Some("^[0-9]+$".to_owned()),
                    ..Default::default()
                },
                ToolArg {
                    name: "target".to_owned(),
                    default: Some("default_tgt".to_owned()),
                    ..Default::default()
                },
            ],
            output_capture: OutputCapture::Stdout,
            ..Default::default()
        };

        let args = json!({
            "lib": true,
            "depth": 3
        });

        let (out, is_err) = execute(&tool, &temp, &args).unwrap();
        assert!(!is_err);
        assert!(out.contains("--lib --depth 3 default_tgt"));
    }

    #[test]
    fn test_enforce_sandbox_in_outer_sandbox() {
        let temp = create_test_dir("sandbox_outer");
        let tool = ToolDefinition {
            name: "test_sandboxed_echo".to_owned(),
            command: vec!["echo".to_owned()],
            sandbox: agy_common::tools::ToolSandboxPolicy {
                enforce_sandbox: true,
                ..Default::default()
            },
            args: vec![ToolArg {
                name: "msg".to_owned(),
                required: true,
                regex: Some("^[a-zA-Z0-9_]+$".to_owned()),
                ..Default::default()
            }],
            output_capture: OutputCapture::Stdout,
            ..Default::default()
        };

        let args = json!({
            "msg": "sandboxed_payload"
        });

        let (out, is_err) = execute_with_detector(&tool, &temp, &args, || true).unwrap();
        assert!(!is_err);
        assert!(out.contains("sandboxed_payload"));
    }

    fn is_bwrap_functional() -> bool {
        if which::which("bwrap").is_err() {
            return false;
        }
        std::process::Command::new("bwrap")
            .args(["--ro-bind", "/", "/", "--", "/bin/sh", "-c", "exit 0"])
            .output()
            .is_ok_and(|out| out.status.success())
    }

    #[test]
    fn test_enforce_sandbox_via_bwrap() {
        if !is_bwrap_functional() || which::which("python3").is_err() {
            return;
        }
        let temp = create_test_dir("sandbox_bwrap");
        let tool = ToolDefinition {
            name: "eval_test".to_owned(),
            command: vec!["python3".to_owned(), "-c".to_owned()],
            sandbox: agy_common::tools::ToolSandboxPolicy {
                enforce_sandbox: true,
                ..Default::default()
            },
            args: vec![ToolArg {
                name: "code".to_owned(),
                required: true,
                flag: None,
                regex: Some("^[^\\x00]+$".to_owned()),
                description: "python code".to_owned(),
                default: None,
                ..Default::default()
            }],
            output_capture: OutputCapture::Stdout,
            ..Default::default()
        };

        let args = json!({
            "code": "print('bwrap_isolation_passed')"
        });

        let (out, is_err) = execute(&tool, &temp, &args).unwrap();
        assert!(!is_err);
        assert!(out.contains("bwrap_isolation_passed"));
    }

    #[test]
    fn test_enforce_sandbox_blocks_network() {
        if !is_bwrap_functional() || which::which("python3").is_err() {
            return;
        }
        let temp = create_test_dir("sandbox_network");
        let tool = ToolDefinition {
            name: "eval_network_test".to_owned(),
            command: vec!["python3".to_owned(), "-c".to_owned()],
            sandbox: agy_common::tools::ToolSandboxPolicy {
                enforce_sandbox: true,
                ..Default::default()
            },
            args: vec![ToolArg {
                name: "code".to_owned(),
                required: true,
                flag: None,
                regex: Some("^[^\\x00]+$".to_owned()),
                description: "python code".to_owned(),
                default: None,
                ..Default::default()
            }],
            output_capture: OutputCapture::Both,
            ..Default::default()
        };

        let args = json!({
            "code": "import socket\ntry:\n    s = socket.create_connection(('8.8.8.8', 53), timeout=0.5)\n    print('CONNECTED')\nexcept Exception as e:\n    print(f'NETWORK_BLOCKED:{type(e).__name__}')"
        });

        let (out, is_err) = execute(&tool, &temp, &args).unwrap();
        assert!(!is_err);
        assert!(
            out.contains("NETWORK_BLOCKED"),
            "Expected network to be blocked in bwrap jail, got: {out}"
        );
        assert!(!out.contains("CONNECTED"));
    }
}
