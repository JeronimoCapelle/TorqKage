//! # agy-common
//!
//! Shared types, process hygiene supervisor, dynamic workspace root resolver,
//! and `ProtoJSON` models for the `agy-suite` workspace.

pub mod diagnostics;
pub mod error;
pub mod gatekeeper;
pub mod models;
pub mod resolver;
pub mod supervisor;

pub use diagnostics::{CompactErrorReport, DiagnosticSpan};
pub use error::{AgyError, Result};
pub use gatekeeper::{GateCheck, GateCheckStatus, evaluate_finisher_readiness};
pub use models::{
    CommonHookPayload, EmptyResponse, HandoffRecord, HookDecision, InjectStep, PipelineHandoff,
    PostInvocationInput, PostInvocationResponse, PostToolUseInput, PreInvocationInput,
    PreInvocationResponse, PreToolResponse, PreToolUseInput, SessionStartInput,
    SessionStartResponse, SystemMessagePayload, StopInput, StopResponse, ToolCallPayload,
};
pub use resolver::WorkspaceResolver;
pub use supervisor::{ChildGuard, SubprocessSupervisor};
