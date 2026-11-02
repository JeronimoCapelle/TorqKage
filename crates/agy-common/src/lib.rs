//! # agy-common
//!
//! Shared types, process hygiene supervisor, dynamic workspace root resolver,
//! and `ProtoJSON` models for the `agy-suite` workspace.

pub mod error;
pub mod gatekeeper;
pub mod jujutsu;
pub mod models;
pub mod resolver;
pub mod sandbox;
pub mod supervisor;

pub use error::{AgyError, Result};
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
pub use supervisor::{ChildGuard, SubprocessSupervisor};
