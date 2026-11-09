//! # tkg-common
//!
//! Shared systems primitives, process hygiene supervisor, dynamic workspace root resolver,
//! and `ProtoJSON` lifecycle models for the `Torqkage` (`tkg`) suite.
//!
//! ## Submodules
//! - [`error`]: Centralized typed error hierarchy ([`TkgError`]).
//! - [`frontmatter`]: YAML frontmatter extraction from Markdown agent manifests ([`AgentFrontmatter`]).
//! - [`gatekeeper`]: Modular admission gates and quality checks ([`GateCheck`], [`VcsConflictCheck`]).
//! - [`handoff`]: Session state handoff recording and wake-up continuity context loader.
//! - [`jujutsu`]: Jujutsu workspace isolation, branch sync, and build cache symlinking ([`JujutsuWorkspace`]).
//! - [`manifest_guard`]: Build manifest protection preventing host IDE proc-macro escape vectors.
//! - [`models`]: `ProtoJSON` schema models for lifecycle hooks, handoffs, and circuit breaker.
//! - [`resolver`]: Workspace and repository root discovery across Jujutsu and Git checkouts ([`WorkspaceResolver`]).
//! - [`sandbox`]: Linux container / outer sandbox detection and Bubblewrap jail builder.
//! - [`subagent_gate`]: Subagent admission gate verifying declared `pre_launch_checks`.
//! - [`supervisor`]: Process hygiene supervisor with orphan prevention (`PR_SET_PDEATHSIG` + `setpgid`).
//! - [`tools`]: Declarative `tools.toml` models, parser, and template inheritance engine.
//! - [`wip`]: Working-in-progress (WIP) working tree change detector and cache tracker.

#![forbid(unsafe_code)]

pub mod content_filter;
pub mod error;
pub mod frontmatter;
pub mod gatekeeper;
pub mod handoff;
pub mod harness;
pub mod jujutsu;
pub mod limits;
pub mod manifest_guard;
pub mod models;
pub mod project;
pub mod resolver;
pub mod sandbox;
pub mod state;
pub mod subagent_gate;
pub mod supervisor;
pub mod tools;
pub mod wip;

pub use content_filter::{
    BlockCommentDelimiters, ContentFilterKind, PathPolicy, SyntaxConfig, check_agent_guard_direct,
    check_agent_path_policies_direct, matches_directory_scope,
    matches_directory_scope_with_containers, path_matches_pattern, strip_comments_and_normalize,
    strip_comments_and_normalize_with_syntax, strip_delimited, strip_delimited_and_normalize,
    strip_tests_and_normalize, strip_tests_and_normalize_with_syntax,
};
pub use error::{Result, TkgError};
pub use frontmatter::{AgentFrontmatter, discover_agent_frontmatters};
pub use gatekeeper::{
    GateCheck, GateCheckStatus, ProjectGateCheck, VcsConflictCheck, WorkspaceCleanCheck,
    evaluate_pre_flight_checks, find_agent_manifest_path, parse_pre_launch_checks,
};
pub use handoff::{
    read_last_handoff_message, read_session_handoff, read_session_handoff_record,
    record_session_handoff,
};
pub use harness::Harness;
pub use jujutsu::JujutsuWorkspace;
pub use limits::{
    DEFAULT_GATE_TIMEOUT_SECS, DEFAULT_HELP_PREVIEW_LINES, DEFAULT_MAN_PREVIEW_LINES,
    DEFAULT_MISSING_BINARIES_PREVIEW, DEFAULT_PREVIEW_LINES, DEFAULT_QUICK_COMMAND_TIMEOUT_SECS,
    DEFAULT_TEST_GATE_TIMEOUT_SECS, DEFAULT_TOOL_TIMEOUT_SECS, DEFAULT_TRUNCATE_LINES,
    DEFAULT_VCS_QUERY_TIMEOUT_SECS, PROCESS_KILL_GRACE_WAIT,
};
pub use manifest_guard::{
    DEFAULT_PROTECTED_MANIFESTS, check_manifest_protection, extract_candidate_paths,
    is_default_fallback_mutation_tool, is_manifest_protected,
};
pub use models::{
    AgentGuard, CircuitBreakerLedger, CommonHookPayload, EmptyResponse, HandoffRecord,
    HookDecision, InjectStep, PipelineHandoff, PostInvocationInput, PostInvocationResponse,
    PostToolUseInput, PreInvocationInput, PreInvocationResponse, PreToolResponse, PreToolUseInput,
    SessionStartInput, SessionStartResponse, StopInput, StopResponse, SystemMessagePayload,
    ToolCallPayload, extract_bracketed_error_code, extract_compiler_error_signature,
};
pub use project::{
    GateDefinition, ProjectConfig, ProjectDocsConfig, ProjectMeta, ProjectRunnerConfig,
    ProjectSandboxConfig, ProjectSecurityConfig, ProjectWorkspaceConfig, SecurityCategory,
    check_dangerous_files_with_security,
};
pub use resolver::WorkspaceResolver;
pub use rustc_hash::{
    FxBuildHasher, FxBuildHasher as FastBuildHasher, FxHashMap, FxHashMap as FastHashMap,
    FxHashSet, FxHashSet as FastHashSet, FxHasher, FxHasher as FastHasher,
};
pub use sandbox::{build_bubblewrap_command, is_inside_container};
pub use state::{TkgStatePaths, atomic_write, resolve_active_agent, set_active_agent};
pub use subagent_gate::{
    SubagentAdmissionResult, check_subagent_admission, inspect_subagent_admission,
};
pub use supervisor::{ChildGuard, SubprocessSupervisor};
pub use wip::{WipCheckResult, check_wip_changes};
