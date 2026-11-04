//! # limits
//!
//! Centralized timeout, output truncation, and process execution thresholds for agy-suite.

use std::time::Duration;

/// Default timeout in seconds for generic tool execution.
pub const DEFAULT_TOOL_TIMEOUT_SECS: u64 = 15;

/// Default timeout in seconds for project quality gates (compilation, linter, formatting).
pub const DEFAULT_GATE_TIMEOUT_SECS: u64 = 30;

/// Default timeout in seconds for test suite compilation and execution gates.
pub const DEFAULT_TEST_GATE_TIMEOUT_SECS: u64 = 45;

/// Default timeout in seconds for quick informational CLI commands (e.g., `--help`, `navi`, `tldr`, `rustc --explain`).
pub const DEFAULT_QUICK_COMMAND_TIMEOUT_SECS: u64 = 5;

/// Default timeout in seconds for VCS queries (Jujutsu and Git queries).
pub const DEFAULT_VCS_QUERY_TIMEOUT_SECS: u64 = 10;

/// Default line count for standard diagnostic output previews and snippets.
pub const DEFAULT_PREVIEW_LINES: usize = 25;

/// Default line count for CLI `--help` output previews to conserve agent context window.
pub const DEFAULT_HELP_PREVIEW_LINES: usize = 50;

/// Default line count for man page synopsis previews.
pub const DEFAULT_MAN_PREVIEW_LINES: usize = 35;

/// Default maximum line limit for tool stdout/stderr output before truncating.
pub const DEFAULT_TRUNCATE_LINES: usize = 120;

/// Maximum number of missing prerequisite binaries displayed in startup diagnostics before "+N more".
pub const DEFAULT_MISSING_BINARIES_PREVIEW: usize = 4;

/// Grace duration allowed for child process group unblocking post-SIGKILL before returning timeout error.
pub const PROCESS_KILL_GRACE_WAIT: Duration = Duration::from_millis(50);
