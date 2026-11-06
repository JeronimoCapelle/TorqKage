//! # agy-common
//!
//! Shared systems primitives, process hygiene supervisor, dynamic workspace root resolver,
//! and `ProtoJSON` lifecycle models for the `agy-suite` workspace.
//!
//! ## Submodules
//! - [`error`]: Centralized typed error hierarchy ([`AgyError`]).
//! - [`frontmatter`]: YAML frontmatter extraction from Markdown agent manifests ([`AgentFrontmatter`]).
//! - [`gatekeeper`]: Modular admission gates and quality checks ([`GateCheck`], [`VcsConflictCheck`]).
//! - [`jujutsu`]: Jujutsu workspace isolation, branch sync, and build cache symlinking ([`JujutsuWorkspace`]).
//! - [`models`]: `ProtoJSON` schema models for Antigravity lifecycle hooks, handoffs, and circuit breaker.
//! - [`resolver`]: Workspace and repository root discovery across Jujutsu and Git checkouts ([`WorkspaceResolver`]).
//! - [`sandbox`]: Linux container / outer sandbox detection and Bubblewrap jail builder.
//! - [`supervisor`]: Process hygiene supervisor with orphan prevention (`PR_SET_PDEATHSIG` + `setpgid`).
//! - [`tools`]: Declarative `tools.toml` models, parser, and template inheritance engine.

pub mod content_filter;
pub mod error;
pub mod fast_hash;
pub mod frontmatter;
pub mod gatekeeper;
pub mod jujutsu;
pub mod limits;
pub mod models;
pub mod project;
pub mod resolver;
pub mod sandbox;
pub mod state;
pub mod supervisor;
pub mod tools;

pub use content_filter::{BlockCommentDelimiters, ContentFilterKind, PathPolicy, SyntaxConfig};
pub use error::{AgyError, Result};
pub use fast_hash::{
    FastBuildHasher, FastHashMap, FastHashSet, FastHasher, fast_map_new, fast_map_with_capacity,
    fast_set_new, fast_set_with_capacity,
};
pub use frontmatter::AgentFrontmatter;
pub use gatekeeper::{
    CargoCompilationCheck, CargoTestCompilationCheck, GateCheck, GateCheckStatus, ProjectGateCheck,
    VcsConflictCheck, WorkspaceCleanCheck, evaluate_finisher_readiness, evaluate_pre_flight_checks,
    find_agent_manifest_path, parse_pre_launch_checks,
};
pub use jujutsu::JujutsuWorkspace;
pub use limits::{
    DEFAULT_GATE_TIMEOUT_SECS, DEFAULT_HELP_PREVIEW_LINES, DEFAULT_MAN_PREVIEW_LINES,
    DEFAULT_MISSING_BINARIES_PREVIEW, DEFAULT_PREVIEW_LINES, DEFAULT_QUICK_COMMAND_TIMEOUT_SECS,
    DEFAULT_TEST_GATE_TIMEOUT_SECS, DEFAULT_TOOL_TIMEOUT_SECS, DEFAULT_TRUNCATE_LINES,
    DEFAULT_VCS_QUERY_TIMEOUT_SECS, PROCESS_KILL_GRACE_WAIT,
};
pub use models::{
    AgentGuard, CircuitBreakerLedger, CommonHookPayload, EmptyResponse, HandoffRecord, HookDecision,
    InjectStep, PipelineHandoff, PostInvocationInput, PostInvocationResponse, PostToolUseInput,
    PreInvocationInput, PreInvocationResponse, PreToolResponse, PreToolUseInput, SessionStartInput,
    SessionStartResponse, SystemMessagePayload, StopInput, StopResponse, ToolCallPayload,
    extract_compiler_error_signature, extract_rustc_error_code,
};
pub use project::{
    GateDefinition, ProjectConfig, ProjectDocsConfig, ProjectMeta, ProjectSandboxConfig,
    ProjectSecurityConfig, ProjectWorkspaceConfig, SecurityCategory,
};
pub use resolver::WorkspaceResolver;
pub use sandbox::{build_bubblewrap_command, is_inside_outer_sandbox};
pub use state::AgyStatePaths;
pub use supervisor::{ChildGuard, SubprocessSupervisor};
