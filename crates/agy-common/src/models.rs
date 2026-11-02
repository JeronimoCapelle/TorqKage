//! `ProtoJSON` models and lifecycle event payload schemas for Antigravity hooks.

use std::path::PathBuf;
use serde::{Deserialize, Serialize};

/// Output decision for Antigravity `PreToolUse` hooks.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum HookDecision {
    Allow,
    Deny,
    Ask,
    ForceAsk,
}

/// Common system metadata sent in `stdin` to every Antigravity hook.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CommonHookPayload {
    #[serde(default)]
    pub conversation_id: Option<String>,
    #[serde(default)]
    pub workspace_paths: Vec<PathBuf>,
    #[serde(default)]
    pub transcript_path: Option<PathBuf>,
    #[serde(default)]
    pub artifact_directory_path: Option<PathBuf>,
    #[serde(default)]
    pub model_name: Option<String>,
    #[serde(default)]
    pub last_user_input: Option<String>,
}

/// Tool call specification contained in tool-related hook inputs.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct ToolCallPayload {
    pub name: String,
    #[serde(default)]
    pub args: serde_json::Value,
}

/// `PreToolUse` input payload received on `stdin`.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PreToolUseInput {
    #[serde(default)]
    pub tool_call: Option<ToolCallPayload>,
    #[serde(default)]
    pub step_idx: Option<u64>,
    #[serde(flatten)]
    pub common: CommonHookPayload,
}

/// `PreToolUse` response emitted on `stdout`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PreToolResponse {
    pub decision: HookDecision,
    #[serde(skip_serializing_if = "Option::is_none", default)]
    pub reason: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none", default)]
    pub permission_overrides: Option<Vec<String>>,
    #[serde(skip_serializing_if = "Option::is_none", default)]
    pub overwrite: Option<serde_json::Value>,
}

impl PreToolResponse {
    /// Constructs an `allow` decision response.
    #[must_use]
    pub const fn allow() -> Self {
        Self {
            decision: HookDecision::Allow,
            reason: None,
            permission_overrides: None,
            overwrite: None,
        }
    }

    /// Constructs a `deny` decision response with the specified reason.
    #[must_use]
    pub fn deny(reason: impl Into<String>) -> Self {
        Self {
            decision: HookDecision::Deny,
            reason: Some(reason.into()),
            permission_overrides: None,
            overwrite: None,
        }
    }

    /// Constructs an `ask` decision response.
    #[must_use]
    pub fn ask(reason: impl Into<String>) -> Self {
        Self {
            decision: HookDecision::Ask,
            reason: Some(reason.into()),
            permission_overrides: None,
            overwrite: None,
        }
    }

    /// Constructs a `force_ask` decision response.
    #[must_use]
    pub fn force_ask(reason: impl Into<String>) -> Self {
        Self {
            decision: HookDecision::ForceAsk,
            reason: Some(reason.into()),
            permission_overrides: None,
            overwrite: None,
        }
    }
}

/// `PostToolUse` input payload received on `stdin`.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PostToolUseInput {
    #[serde(default)]
    pub tool_call: Option<ToolCallPayload>,
    #[serde(default)]
    pub step_idx: Option<u64>,
    #[serde(default)]
    pub error: Option<String>,
    #[serde(flatten)]
    pub common: CommonHookPayload,
}

/// Empty response emitted on `stdout` when no action or state change is returned.
#[allow(clippy::empty_structs_with_brackets, reason = "Explicit empty object {} required by ProtoJSON")]
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct EmptyResponse {}

/// System message wrapper required by the Antigravity conversation protocol.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SystemMessagePayload {
    #[serde(rename = "systemMessage")]
    pub text: String,
    #[serde(skip_serializing_if = "Option::is_none", default)]
    pub metadata: Option<serde_json::Value>,
}

/// Injectable step for `SessionStart`, `PreInvocation`, and `PostInvocation` hooks.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(untagged)]
pub enum InjectStep {
    System {
        #[serde(rename = "systemMessage")]
        system_message: SystemMessagePayload,
    },
    User {
        #[serde(rename = "userMessage")]
        user_message: String,
    },
    Ephemeral {
        #[serde(rename = "ephemeralMessage")]
        ephemeral_message: String,
    },
}

impl InjectStep {
    /// Injects an ephemeral notification (dropped after current turn).
    #[must_use]
    pub fn ephemeral(msg: impl Into<String>) -> Self {
        Self::Ephemeral {
            ephemeral_message: msg.into(),
        }
    }

    /// Injects a persistent user message into the trajectory.
    #[must_use]
    pub fn user(msg: impl Into<String>) -> Self {
        Self::User {
            user_message: msg.into(),
        }
    }

    /// Injects a persistent system message into the trajectory.
    #[must_use]
    pub fn system(msg: impl Into<String>) -> Self {
        Self::System {
            system_message: SystemMessagePayload {
                text: msg.into(),
                metadata: None,
            },
        }
    }

    /// Injects a system message carrying custom metadata (e.g. user markdown).
    #[must_use]
    pub fn system_with_metadata(msg: impl Into<String>, metadata: serde_json::Value) -> Self {
        Self::System {
            system_message: SystemMessagePayload {
                text: msg.into(),
                metadata: Some(metadata),
            },
        }
    }
}

/// `SessionStart` input payload received on `stdin`.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SessionStartInput {
    #[serde(flatten)]
    pub common: CommonHookPayload,
}

/// `SessionStart` response payload emitted on `stdout`.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SessionStartResponse {
    #[serde(default)]
    pub inject_steps: Vec<InjectStep>,
}

/// `PreInvocation` input payload received on `stdin`.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PreInvocationInput {
    #[serde(default)]
    pub invocation_num: Option<u64>,
    #[serde(default)]
    pub initial_num_steps: Option<u64>,
    #[serde(flatten)]
    pub common: CommonHookPayload,
}

/// `PreInvocation` response payload emitted on `stdout`.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PreInvocationResponse {
    #[serde(skip_serializing_if = "Option::is_none", default)]
    pub inject_steps: Option<Vec<InjectStep>>,
}

impl PreInvocationResponse {
    /// Constructs an empty response (`{}`) burning zero tokens.
    #[must_use]
    pub const fn empty() -> Self {
        Self { inject_steps: None }
    }

    /// Constructs a response injecting an ephemeral notice.
    #[must_use]
    pub fn ephemeral(msg: impl Into<String>) -> Self {
        Self {
            inject_steps: Some(vec![InjectStep::ephemeral(msg)]),
        }
    }

    /// Constructs a response injecting arbitrary steps.
    #[must_use]
    pub const fn with_steps(steps: Vec<InjectStep>) -> Self {
        Self {
            inject_steps: Some(steps),
        }
    }
}

/// `PostInvocation` input payload received on `stdin`.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PostInvocationInput {
    #[serde(default)]
    pub invocation_num: Option<u64>,
    #[serde(default)]
    pub initial_num_steps: Option<u64>,
    #[serde(flatten)]
    pub common: CommonHookPayload,
}

/// `PostInvocation` response payload emitted on `stdout`.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PostInvocationResponse {
    #[serde(skip_serializing_if = "Option::is_none", default)]
    pub inject_steps: Option<Vec<InjectStep>>,
    #[serde(skip_serializing_if = "Option::is_none", default)]
    pub termination_behavior: Option<String>,
}

/// `Stop` input payload received on `stdin`.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct StopInput {
    #[serde(default)]
    pub execution_num: Option<u64>,
    #[serde(default)]
    pub termination_reason: Option<String>,
    #[serde(default)]
    pub error: Option<String>,
    #[serde(default)]
    pub fully_idle: Option<bool>,
    #[serde(flatten)]
    pub common: CommonHookPayload,
}

/// `Stop` response payload emitted on `stdout`.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct StopResponse {
    #[serde(skip_serializing_if = "Option::is_none", default)]
    pub decision: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none", default)]
    pub reason: Option<String>,
}

impl StopResponse {
    /// Constructs a response permitting the execution loop to terminate cleanly (`{}`).
    #[must_use]
    pub const fn allow() -> Self {
        Self {
            decision: None,
            reason: None,
        }
    }

    /// Constructs a response instructing the execution loop to continue.
    #[must_use]
    pub fn force_continue(reason: impl Into<String>) -> Self {
        Self {
            decision: Some("continue".to_owned()),
            reason: Some(reason.into()),
        }
    }
}

/// Ephemeral session handoff metadata written to `.pipeline/last_handoff.md`.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct HandoffRecord {
    pub previous_agent: String,
    pub timestamp: String,
    pub modified_files: Vec<String>,
    pub status: String,
}

impl HandoffRecord {
    /// Creates a new `HandoffRecord` with the current ISO-8601 UTC timestamp.
    #[must_use]
    pub fn new(
        previous_agent: impl Into<String>,
        modified_files: Vec<String>,
        status: impl Into<String>,
    ) -> Self {
        Self {
            previous_agent: previous_agent.into(),
            timestamp: format!("{}", jiff::Timestamp::now()),
            modified_files,
            status: status.into(),
        }
    }

    /// Formats the handoff record into a clean Markdown block for `.pipeline/last_handoff.md`.
    #[must_use]
    pub fn to_markdown(&self) -> String {
        let files = if self.modified_files.is_empty() {
            "None (clean)".to_owned()
        } else {
            self.modified_files.join(", ")
        };

        format!(
            "- Previous Agent: {}\n- Timestamp: {}\n- Modified Files: {}\n- Status: {}\n",
            self.previous_agent, self.timestamp, files, self.status
        )
    }

    /// Parses a handoff record from `.pipeline/last_handoff.md` content.
    #[must_use]
    pub fn from_markdown(content: &str) -> Option<Self> {
        let mut record = Self::default();
        let mut found_any = false;

        for line in content.lines() {
            let trimmed = line.trim();
            if let Some(val) = trimmed.strip_prefix("- Previous Agent:") {
                val.trim().clone_into(&mut record.previous_agent);
                found_any = true;
            } else if let Some(val) = trimmed.strip_prefix("- Timestamp:") {
                val.trim().clone_into(&mut record.timestamp);
                found_any = true;
            } else if let Some(val) = trimmed.strip_prefix("- Modified Files:") {
                let list = val.trim();
                if list != "None (clean)" && !list.is_empty() {
                    record.modified_files = list
                        .split(',')
                        .map(|s| s.trim().to_owned())
                        .filter(|s| !s.is_empty())
                        .collect();
                }
                found_any = true;
            } else if let Some(val) = trimmed.strip_prefix("- Status:") {
                val.trim().clone_into(&mut record.status);
                found_any = true;
            }
        }

        if found_any {
            Some(record)
        } else {
            None
        }
    }
}

/// Structured inter-agent pipeline handoff manifest stored at `.pipeline/handoff.json`.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct PipelineHandoff {
    pub from_agent: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub target_agent: Option<String>,
    pub timestamp: String,
    #[serde(default)]
    pub modified_files: Vec<String>,
    #[serde(default)]
    pub deliverables: Vec<String>,
    pub summary: String,
    pub status: String,
}

impl PipelineHandoff {
    /// Constructs a new `PipelineHandoff` with the current UTC timestamp.
    #[must_use]
    pub fn new(
        from_agent: impl Into<String>,
        target_agent: Option<String>,
        summary: impl Into<String>,
        modified_files: Vec<String>,
        deliverables: Vec<String>,
        status: impl Into<String>,
    ) -> Self {
        Self {
            from_agent: from_agent.into(),
            target_agent,
            timestamp: format!("{}", jiff::Timestamp::now()),
            modified_files,
            deliverables,
            summary: summary.into(),
            status: status.into(),
        }
    }

    /// Formats an actionable prompt block for session context injection.
    #[allow(clippy::format_push_string, reason = "Constructing human-readable prompt string")]
    #[must_use]
    pub fn to_injection_message(&self) -> String {
        let mut msg = format!(
            "[PIPELINE CONTEXT HANDOFF]: Work handed off from '{}'",
            self.from_agent
        );
        if let Some(ref target) = self.target_agent {
            msg.push_str(&format!(" to '{target}'"));
        }
        msg.push_str(&format!(" (Status: {}).\n", self.status));
        msg.push_str(&format!("Summary: {}\n", self.summary));

        if !self.deliverables.is_empty() {
            msg.push_str(&format!("Deliverables / Specs: {}.\n", self.deliverables.join(", ")));
        }
        if !self.modified_files.is_empty() {
            msg.push_str(&format!("Modified Files: {}.\n", self.modified_files.join(", ")));
        }
        msg
    }
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
    fn test_pre_tool_use_roundtrip() {
        let json_input = r#"{
            "toolCall": {
                "name": "run_command",
                "args": { "CommandLine": "cargo test" }
            },
            "stepIdx": 19,
            "conversationId": "c-1234",
            "workspacePaths": ["/home/test/project"],
            "transcriptPath": "/home/test/project/transcript.jsonl",
            "artifactDirectoryPath": "/home/test/project/artifacts",
            "modelName": "gemini-3.8-flash",
            "lastUserInput": "Check clippy"
        }"#;

        let parsed: PreToolUseInput = serde_json::from_str(json_input).expect("deserialize failed");
        assert_eq!(parsed.step_idx, Some(19));
        assert_eq!(parsed.common.conversation_id.as_deref(), Some("c-1234"));
        let tool = parsed.tool_call.expect("tool call missing");
        assert_eq!(tool.name, "run_command");
        assert_eq!(
            tool.args.get("CommandLine").and_then(serde_json::Value::as_str),
            Some("cargo test")
        );

        // Allow output
        let allow_resp = PreToolResponse::allow();
        let allow_json = serde_json::to_string(&allow_resp).expect("serialize failed");
        assert_eq!(allow_json, r#"{"decision":"allow"}"#);

        // Deny output
        let deny_resp = PreToolResponse::deny("Rule violation");
        let deny_json = serde_json::to_string(&deny_resp).expect("serialize failed");
        assert_eq!(deny_json, r#"{"decision":"deny","reason":"Rule violation"}"#);
    }

    #[test]
    fn test_inject_step_serialization() {
        // Ephemeral
        let ephemeral = InjectStep::ephemeral("Files modified on disk");
        let eph_json = serde_json::to_string(&ephemeral).expect("serialize failed");
        assert_eq!(eph_json, r#"{"ephemeralMessage":"Files modified on disk"}"#);

        // User
        let user_msg = InjectStep::user("Proceed with next step");
        let user_json = serde_json::to_string(&user_msg).expect("serialize failed");
        assert_eq!(user_json, r#"{"userMessage":"Proceed with next step"}"#);

        // System
        let sys_msg = InjectStep::system("System initialisation complete");
        let sys_json = serde_json::to_string(&sys_msg).expect("serialize failed");
        assert_eq!(sys_json, r#"{"systemMessage":{"systemMessage":"System initialisation complete"}}"#);

        // Roundtrip deserialization
        let parsed_eph: InjectStep = serde_json::from_str(&eph_json).expect("deserialize failed");
        assert_eq!(parsed_eph, ephemeral);

        let parsed_user: InjectStep = serde_json::from_str(&user_json).expect("deserialize failed");
        assert_eq!(parsed_user, user_msg);

        let parsed_sys: InjectStep = serde_json::from_str(&sys_json).expect("deserialize failed");
        assert_eq!(parsed_sys, sys_msg);
    }

    #[test]
    fn test_pre_invocation_empty_burns_zero_tokens() {
        let resp = PreInvocationResponse::empty();
        let serialized = serde_json::to_string(&resp).expect("serialize failed");
        assert_eq!(serialized, "{}");

        let with_notice = PreInvocationResponse::ephemeral("WIP detected");
        let notice_json = serde_json::to_string(&with_notice).expect("serialize failed");
        assert_eq!(notice_json, r#"{"injectSteps":[{"ephemeralMessage":"WIP detected"}]}"#);
    }

    #[test]
    fn test_stop_response_allow_is_empty() {
        let stop_allow = StopResponse::allow();
        let serialized = serde_json::to_string(&stop_allow).expect("serialize failed");
        assert_eq!(serialized, "{}");

        let stop_cont = StopResponse::force_continue("Tests still running in background");
        let cont_json = serde_json::to_string(&stop_cont).expect("serialize failed");
        assert_eq!(
            cont_json,
            r#"{"decision":"continue","reason":"Tests still running in background"}"#
        );
    }

    #[test]
    fn test_handoff_markdown_roundtrip() {
        let record = HandoffRecord {
            previous_agent: "planner".to_owned(),
            timestamp: "2026-11-01T15:00:00Z".to_owned(),
            modified_files: vec!["ARCHITECTURE.md".to_owned(), "src/lib.rs".to_owned()],
            status: "WIP".to_owned(),
        };

        let md = record.to_markdown();
        let parsed = HandoffRecord::from_markdown(&md).expect("markdown parse failed");
        assert_eq!(parsed, record);
    }
}
