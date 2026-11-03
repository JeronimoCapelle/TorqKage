//! End-to-end integration tests for `agy-hooks` lifecycle hook CLI.

#![allow(
    clippy::expect_used,
    clippy::unwrap_used,
    clippy::panic,
    clippy::tests_outside_test_module,
    clippy::collapsible_if,
    reason = "Integration test assertions"
)]

use std::io::Write;
use std::process::{Command, Stdio};
use agy_common::{
    PreInvocationInput, PreInvocationResponse, SessionStartInput, SessionStartResponse, StopInput,
    StopResponse,
};

fn run_hook_subcommand(subcmd: &str, stdin_json: Option<&str>) -> (bool, String, String) {
    let mut cmd = Command::new(env!("CARGO_BIN_EXE_agy-hooks"));
    cmd.arg(subcmd);
    cmd.stdout(Stdio::piped());
    cmd.stderr(Stdio::piped());

    if stdin_json.is_some() {
        cmd.stdin(Stdio::piped());
    } else {
        cmd.stdin(Stdio::null());
    }

    let mut child = cmd.spawn().expect("failed to spawn agy-hooks binary");

    if let Some(json) = stdin_json {
        if let Some(mut stdin) = child.stdin.take() {
            stdin.write_all(json.as_bytes()).expect("write to stdin failed");
        }
    }

    let output = child.wait_with_output().expect("failed to read hook output");
    let stdout = String::from_utf8_lossy(&output.stdout).trim().to_owned();
    let stderr = String::from_utf8_lossy(&output.stderr).trim().to_owned();

    (output.status.success(), stdout, stderr)
}

#[test]
fn test_record_handoff_and_inject_wake_roundtrip_e2e() {
    let temp_workspace = std::env::temp_dir().join(format!("agy_test_ws_{}", std::process::id()));
    std::fs::create_dir_all(&temp_workspace).expect("create temp workspace");

    // 1. Run record-handoff
    let stop_input = StopInput {
        execution_num: Some(1),
        termination_reason: Some("NO_TOOL_CALL".to_owned()),
        error: None,
        fully_idle: Some(true),
        common: agy_common::CommonHookPayload {
            conversation_id: Some("test-handoff-conv".to_owned()),
            workspace_paths: vec![temp_workspace.clone()],
            transcript_path: None,
            artifact_directory_path: None,
            model_name: Some("planner".to_owned()),
            last_user_input: None,
        },
    };

    let stop_json = serde_json::to_string(&stop_input).expect("serialize stop input");
    let (success_stop, stdout_stop, _) = run_hook_subcommand("record-handoff", Some(&stop_json));
    assert!(success_stop);
    let stop_resp: StopResponse = serde_json::from_str(&stdout_stop).expect("valid StopResponse JSON");
    assert_eq!(stop_resp.decision, None); // {} permits stop

    // Check that .agy/state/last_handoff.md was created
    let handoff_file = agy_common::AgyStatePaths::new(&temp_workspace).handoff_md_file();
    assert!(handoff_file.exists());

    // 2. Run inject-wake
    let start_input = SessionStartInput {
        common: agy_common::CommonHookPayload {
            conversation_id: Some("test-wake-conv".to_owned()),
            workspace_paths: vec![temp_workspace.clone()],
            transcript_path: None,
            artifact_directory_path: None,
            model_name: Some("copilot".to_owned()),
            last_user_input: None,
        },
    };

    let start_json = serde_json::to_string(&start_input).expect("serialize start input");
    let (success_wake, stdout_wake, _) = run_hook_subcommand("inject-wake", Some(&start_json));
    assert!(success_wake);

    let wake_resp: SessionStartResponse = serde_json::from_str(&stdout_wake).expect("valid SessionStartResponse");
    assert_eq!(wake_resp.inject_steps.len(), 1);

    // Clean up temp workspace
    drop(std::fs::remove_dir_all(&temp_workspace));
}

#[test]
fn test_detect_wip_consecutive_burns_zero_tokens_e2e() {
    let payload = PreInvocationInput {
        invocation_num: Some(1),
        initial_num_steps: Some(10),
        common: agy_common::CommonHookPayload {
            conversation_id: Some(format!("test-wip-conv-{}", std::process::id())),
            workspace_paths: vec![std::env::temp_dir()],
            transcript_path: None,
            artifact_directory_path: None,
            model_name: Some("auto".to_owned()),
            last_user_input: Some("proceed".to_owned()),
        },
    };

    let input_json = serde_json::to_string(&payload).expect("serialize payload");

    // First call
    let (success1, _, _) = run_hook_subcommand("detect-wip", Some(&input_json));
    assert!(success1);

    // Immediate second call should be clean/unchanged -> burns 0 tokens ({})
    let (success2, stdout2, _) = run_hook_subcommand("detect-wip", Some(&input_json));
    assert!(success2);
    let resp2: PreInvocationResponse = serde_json::from_str(&stdout2).expect("valid PreInvocationResponse");
    assert_eq!(resp2.inject_steps, None);
    assert_eq!(stdout2, "{}");
}

#[test]
fn test_check_conflicts_e2e() {
    let (success, stdout, _) = run_hook_subcommand("check-conflicts", None);
    assert!(success);
    let resp: agy_common::PreToolResponse = serde_json::from_str(&stdout).expect("valid PreToolResponse");
    // Clean working repo should allow
    assert_eq!(resp.decision, agy_common::HookDecision::Allow);
}

#[test]
fn test_pre_tool_use_read_only_allowed_e2e() {
    let payload = agy_common::PreToolUseInput {
        tool_call: Some(agy_common::models::ToolCallPayload {
            name: "read_project_file".to_owned(),
            args: serde_json::json!({"path": "src/lib.rs"}),
        }),
        ..Default::default()
    };

    let input_json = serde_json::to_string(&payload).expect("serialize payload");
    let (success, stdout, _) = run_hook_subcommand("pre-tool-use", Some(&input_json));
    assert!(success);
    let resp: agy_common::PreToolResponse = serde_json::from_str(&stdout).expect("valid PreToolResponse");
    assert_eq!(resp.decision, agy_common::HookDecision::Allow);
}

#[test]
fn test_pre_tool_use_mutating_asks_e2e() {
    let payload = agy_common::PreToolUseInput {
        tool_call: Some(agy_common::models::ToolCallPayload {
            name: "modify_code".to_owned(),
            args: serde_json::json!({"path": "src/lib.rs"}),
        }),
        ..Default::default()
    };

    let input_json = serde_json::to_string(&payload).expect("serialize payload");
    let (success, stdout, _) = run_hook_subcommand("pre-tool-use", Some(&input_json));
    assert!(success);
    let resp: agy_common::PreToolResponse = serde_json::from_str(&stdout).expect("valid PreToolResponse");
    assert_eq!(resp.decision, agy_common::HookDecision::Ask);
}
