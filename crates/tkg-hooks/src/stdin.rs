//! Hook wire dialect abstraction and `stdin`/`stdout` JSON helpers for lifecycle hooks.
//!
//! Each execution harness (Antigravity, Claude Code, `OpenCode`, etc.) can speak a slightly
//! different JSON dialect over `stdin`/`stdout`. [`HookDialect`] isolates wire translation so
//! all hook evaluators operate purely on the canonical [`tkg_common`] types.

use std::io::IsTerminal;

use tkg_common::{
    CommonHookPayload, PreInvocationInput, PreInvocationResponse, PreToolResponse, PreToolUseInput,
    SessionStartInput, SessionStartResponse, StopInput, StopResponse, ToolCallPayload,
};

/// Wire-protocol adapter translating between a harness's hook JSON and Torqkage's canonical hook types.
pub(crate) trait HookDialect: Send + Sync {
    /// Parses a raw `stdin` JSON string into a canonical [`PreToolUseInput`].
    fn parse_pre_tool_use(&self, raw: &str) -> Result<PreToolUseInput, serde_json::Error> {
        serde_json::from_str(raw)
    }

    /// Formats a canonical [`PreToolResponse`] into the harness's `stdout` JSON string.
    fn format_pre_tool_use(&self, resp: &PreToolResponse) -> Result<String, serde_json::Error> {
        serde_json::to_string(resp)
    }

    /// Parses a raw `stdin` JSON string into a canonical [`PreInvocationInput`].
    fn parse_pre_invocation(&self, raw: &str) -> Result<PreInvocationInput, serde_json::Error> {
        serde_json::from_str(raw)
    }

    /// Formats a canonical [`PreInvocationResponse`] into the harness's `stdout` JSON string.
    fn format_pre_invocation(
        &self,
        resp: &PreInvocationResponse,
    ) -> Result<String, serde_json::Error> {
        serde_json::to_string(resp)
    }

    /// Parses a raw `stdin` JSON string into a canonical [`StopInput`].
    fn parse_stop(&self, raw: &str) -> Result<StopInput, serde_json::Error> {
        serde_json::from_str(raw)
    }

    /// Formats a canonical [`StopResponse`] into the harness's `stdout` JSON string.
    fn format_stop(&self, resp: &StopResponse) -> Result<String, serde_json::Error> {
        serde_json::to_string(resp)
    }

    /// Parses a raw `stdin` JSON string into a canonical [`SessionStartInput`].
    fn parse_session_start(&self, raw: &str) -> Result<SessionStartInput, serde_json::Error> {
        serde_json::from_str(raw)
    }

    /// Formats a canonical [`SessionStartResponse`] into the harness's `stdout` JSON string.
    fn format_session_start(
        &self,
        resp: &SessionStartResponse,
    ) -> Result<String, serde_json::Error> {
        serde_json::to_string(resp)
    }
}

/// Default `ProtoJSON` dialect used by Antigravity and generic harnesses.
pub(crate) struct ProtoJsonDialect;

impl HookDialect for ProtoJsonDialect {
    fn parse_pre_tool_use(&self, raw: &str) -> Result<PreToolUseInput, serde_json::Error> {
        let mut parsed: PreToolUseInput = serde_json::from_str(raw)?;
        // Also normalize Claude Code / snake_case `tool_name` + `tool_input` if `toolCall` was omitted
        if parsed.tool_call.is_none()
            && let Ok(val) = serde_json::from_str::<serde_json::Value>(raw)
            && let Some(name) = val.get("tool_name").and_then(serde_json::Value::as_str)
        {
            let args = val
                .get("tool_input")
                .cloned()
                .unwrap_or_else(|| serde_json::json!({}));
            parsed.tool_call = Some(ToolCallPayload {
                name: name.to_owned(),
                args,
            });
            if parsed.common.workspace_paths.is_empty()
                && let Some(cwd) = val.get("cwd").and_then(serde_json::Value::as_str)
            {
                parsed.common = CommonHookPayload {
                    workspace_paths: vec![std::path::PathBuf::from(cwd)],
                    ..parsed.common
                };
            }
        }
        Ok(parsed)
    }
}

/// Returns the active [`HookDialect`].
// TODO(harness): Dispatch to dedicated ClaudeCodeDialect / OpenCodeDialect response formatters
// once per-harness hook output schemas (e.g., Claude Code `hookSpecificOutput` / exit code 2) are wired.
#[must_use]
pub(crate) fn active_dialect() -> &'static dyn HookDialect {
    &ProtoJsonDialect
}

fn read_raw_stdin() -> Option<String> {
    if std::io::stdin().is_terminal() {
        return None;
    }
    let mut buffer = String::new();
    if std::io::Read::read_to_string(&mut std::io::stdin(), &mut buffer).is_err() {
        return None;
    }
    let trimmed = buffer.trim();
    if trimmed.is_empty() {
        None
    } else {
        Some(trimmed.to_owned())
    }
}

/// Reads and deserializes a `PreToolUseInput` payload from `stdin` via the active [`HookDialect`].
#[must_use]
pub(crate) fn read_pre_tool_use_input() -> PreToolUseInput {
    let Some(raw) = read_raw_stdin() else {
        return PreToolUseInput::default();
    };
    match active_dialect().parse_pre_tool_use(&raw) {
        Ok(val) => val,
        Err(err) => {
            eprintln!("tkg-hooks: warning: failed to parse PreToolUse stdin payload: {err}");
            PreToolUseInput::default()
        },
    }
}

/// Emits a `PreToolResponse` to `stdout` via the active [`HookDialect`].
pub(crate) fn write_pre_tool_use_response(resp: &PreToolResponse) {
    if let Ok(json) = active_dialect().format_pre_tool_use(resp) {
        println!("{json}");
    }
}

/// Reads and deserializes a `PreInvocationInput` payload from `stdin` via the active [`HookDialect`].
#[must_use]
pub(crate) fn read_pre_invocation_input() -> PreInvocationInput {
    let Some(raw) = read_raw_stdin() else {
        return PreInvocationInput::default();
    };
    match active_dialect().parse_pre_invocation(&raw) {
        Ok(val) => val,
        Err(err) => {
            eprintln!("tkg-hooks: warning: failed to parse PreInvocation stdin payload: {err}");
            PreInvocationInput::default()
        },
    }
}

/// Emits a `PreInvocationResponse` to `stdout` via the active [`HookDialect`].
pub(crate) fn write_pre_invocation_response(resp: &PreInvocationResponse) {
    if let Ok(json) = active_dialect().format_pre_invocation(resp) {
        println!("{json}");
    }
}

/// Reads and deserializes a `StopInput` payload from `stdin` via the active [`HookDialect`].
#[must_use]
pub(crate) fn read_stop_input() -> StopInput {
    let Some(raw) = read_raw_stdin() else {
        return StopInput::default();
    };
    match active_dialect().parse_stop(&raw) {
        Ok(val) => val,
        Err(err) => {
            eprintln!("tkg-hooks: warning: failed to parse Stop stdin payload: {err}");
            StopInput::default()
        },
    }
}

/// Emits a `StopResponse` to `stdout` via the active [`HookDialect`].
pub(crate) fn write_stop_response(resp: &StopResponse) {
    if let Ok(json) = active_dialect().format_stop(resp) {
        println!("{json}");
    }
}

/// Reads and deserializes a `SessionStartInput` payload from `stdin` via the active [`HookDialect`].
#[must_use]
pub(crate) fn read_session_start_input() -> SessionStartInput {
    let Some(raw) = read_raw_stdin() else {
        return SessionStartInput::default();
    };
    match active_dialect().parse_session_start(&raw) {
        Ok(val) => val,
        Err(err) => {
            eprintln!("tkg-hooks: warning: failed to parse SessionStart stdin payload: {err}");
            SessionStartInput::default()
        },
    }
}

/// Emits a `SessionStartResponse` to `stdout` via the active [`HookDialect`].
pub(crate) fn write_session_start_response(resp: &SessionStartResponse) {
    if let Ok(json) = active_dialect().format_session_start(resp) {
        println!("{json}");
    }
}

#[cfg(test)]
#[allow(clippy::unwrap_used, reason = "Unit test assertions")]
mod tests {
    use super::*;

    #[test]
    fn test_dialect_normalizes_claude_code_pre_tool_use() {
        let raw = r#"{
            "tool_name": "Write",
            "tool_input": { "file_path": "src/lib.rs", "content": "fn main() {}" },
            "cwd": "/tmp/ws"
        }"#;
        let parsed = active_dialect().parse_pre_tool_use(raw).unwrap();
        let tc = parsed.tool_call.unwrap();
        assert_eq!(tc.name, "Write");
        assert_eq!(
            parsed.common.workspace_paths.first(),
            Some(&std::path::PathBuf::from("/tmp/ws"))
        );
    }
}
