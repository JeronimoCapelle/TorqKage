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

pub mod error;
pub mod frontmatter;
pub mod gatekeeper;
pub mod jujutsu;
pub mod models;
pub mod resolver;
pub mod sandbox;
pub mod state;
pub mod supervisor;
pub mod tools;

pub use error::{AgyError, Result};
pub use frontmatter::AgentFrontmatter;
pub use gatekeeper::{
    CargoCompilationCheck, CargoTestCompilationCheck, GateCheck, GateCheckStatus, VcsConflictCheck,
    evaluate_finisher_readiness, evaluate_pre_flight_checks, find_agent_manifest_path,
    parse_pre_launch_checks,
};
pub use jujutsu::JujutsuWorkspace;
pub use models::{
    CircuitBreakerLedger, CommonHookPayload, EmptyResponse, HandoffRecord, HookDecision,
    InjectStep, PipelineHandoff, PostInvocationInput, PostInvocationResponse, PostToolUseInput,
    PreInvocationInput, PreInvocationResponse, PreToolResponse, PreToolUseInput, SessionStartInput,
    SessionStartResponse, SystemMessagePayload, StopInput, StopResponse, ToolCallPayload,
    extract_rustc_error_code,
};
pub use resolver::WorkspaceResolver;
pub use sandbox::{build_bubblewrap_command, is_inside_outer_sandbox};
pub use state::AgyStatePaths;
pub use supervisor::{ChildGuard, SubprocessSupervisor};
