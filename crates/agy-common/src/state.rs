//! Centralized state and workspace path resolution for the `agy-suite` ecosystem.
//!
//! All runtime state, isolated workspaces, and inter-agent artifacts are
//! consolidated under the `.agy/` root directory in the workspace root:
//! - `.agy/state/`: Ephemeral runtime state (circuit breaker ledger, session handoffs, active agent bridges)
//! - `.agy/workspaces/`: Isolated subagent Jujutsu worktrees (`.agy/workspaces/<agent>`)
//! - `.agy/artifacts/`: Checkpoints, performance profiles, and generated deliverables

use std::path::{Path, PathBuf};

/// Path resolver for the `.agy/` state repository.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AgyStatePaths {
    workspace_root: PathBuf,
}

impl AgyStatePaths {
    pub const ROOT_DIR: &'static str = ".agy";
    pub const STATE_DIR: &'static str = "state";
    pub const WORKSPACES_DIR: &'static str = "workspaces";
    pub const ARTIFACTS_DIR: &'static str = "artifacts";

    pub const CIRCUIT_BREAKER_FILE: &'static str = "circuit_breaker.json";
    pub const HANDOFF_JSON_FILE: &'static str = "handoff.json";
    pub const HANDOFF_MD_FILE: &'static str = "last_handoff.md";
    pub const ACTIVE_AGENT_FILE: &'static str = "active_agent.json";
    pub const CHECKPOINT_FILE: &'static str = "checkpoint.json";

    /// Constructs an `AgyStatePaths` instance rooted at `workspace_root`.
    #[must_use]
    pub fn new(workspace_root: impl Into<PathBuf>) -> Self {
        Self {
            workspace_root: workspace_root.into(),
        }
    }

    /// Returns the workspace root path.
    #[must_use]
    pub fn workspace_root(&self) -> &Path {
        &self.workspace_root
    }

    /// The base directory `.agy` under the workspace root.
    #[must_use]
    pub fn root(&self) -> PathBuf {
        self.workspace_root.join(Self::ROOT_DIR)
    }

    /// The `.agy/state` directory for ephemeral runtime state.
    #[must_use]
    pub fn state_dir(&self) -> PathBuf {
        self.root().join(Self::STATE_DIR)
    }

    /// The `.agy/workspaces` directory for isolated subagent Jujutsu workspaces.
    #[must_use]
    pub fn workspaces_dir(&self) -> PathBuf {
        self.root().join(Self::WORKSPACES_DIR)
    }

    /// Path to a specific subagent's isolated workspace: `.agy/workspaces/<agent>`.
    #[must_use]
    pub fn agent_workspace(&self, agent: &str) -> PathBuf {
        self.workspaces_dir().join(agent)
    }

    /// The `.agy/artifacts` directory for durable generated deliverables.
    #[must_use]
    pub fn artifacts_dir(&self) -> PathBuf {
        self.root().join(Self::ARTIFACTS_DIR)
    }

    /// Resolves the circuit breaker ledger file path.
    /// Checks `.agy/state/circuit_breaker.json`, falling back to `.pipeline/circuit_breaker.json` for reading.
    #[must_use]
    pub fn circuit_breaker_file(&self) -> PathBuf {
        let modern = self.state_dir().join(Self::CIRCUIT_BREAKER_FILE);
        if modern.exists() {
            return modern;
        }
        let legacy = self.workspace_root.join(".pipeline").join(Self::CIRCUIT_BREAKER_FILE);
        if legacy.exists() {
            return legacy;
        }
        modern
    }

    /// Resolves the structured handoff JSON file path.
    /// Checks `.agy/state/handoff.json`, falling back to `.pipeline/handoff.json` for reading.
    #[must_use]
    pub fn handoff_json_file(&self) -> PathBuf {
        let modern = self.state_dir().join(Self::HANDOFF_JSON_FILE);
        if modern.exists() {
            return modern;
        }
        let legacy = self.workspace_root.join(".pipeline").join(Self::HANDOFF_JSON_FILE);
        if legacy.exists() {
            return legacy;
        }
        modern
    }

    /// Resolves the human-readable handoff Markdown file path.
    /// Checks `.agy/state/last_handoff.md`, falling back to `.pipeline/last_handoff.md` for reading.
    #[must_use]
    pub fn handoff_md_file(&self) -> PathBuf {
        let modern = self.state_dir().join(Self::HANDOFF_MD_FILE);
        if modern.exists() {
            return modern;
        }
        let legacy = self.workspace_root.join(".pipeline").join(Self::HANDOFF_MD_FILE);
        if legacy.exists() {
            return legacy;
        }
        modern
    }

    /// Resolves the active agent bridge file path (`active_agent.json`).
    #[must_use]
    pub fn active_agent_file(&self) -> PathBuf {
        let modern = self.state_dir().join(Self::ACTIVE_AGENT_FILE);
        if modern.exists() {
            return modern;
        }
        let legacy = self.workspace_root.join(".pipeline").join(Self::ACTIVE_AGENT_FILE);
        if legacy.exists() {
            return legacy;
        }
        modern
    }

    /// Resolves the session checkpoint file path (`checkpoint.json`).
    #[must_use]
    pub fn checkpoint_file(&self) -> PathBuf {
        let modern = self.artifacts_dir().join(Self::CHECKPOINT_FILE);
        if modern.exists() {
            return modern;
        }
        let legacy = self.workspace_root.join(".pipeline").join(Self::CHECKPOINT_FILE);
        if legacy.exists() {
            return legacy;
        }
        modern
    }

    /// Ensures that the core `.agy/` state directories exist on disk.
    ///
    /// # Errors
    /// Returns `std::io::Error` if directory creation fails.
    pub fn ensure_directories(&self) -> std::io::Result<()> {
        std::fs::create_dir_all(self.state_dir())?;
        std::fs::create_dir_all(self.workspaces_dir())?;
        std::fs::create_dir_all(self.artifacts_dir())?;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    #[allow(clippy::expect_used, reason = "Unit test assertions")]
    fn test_agy_state_paths_structure() {
        let root = PathBuf::from("/home/user/project");
        let paths = AgyStatePaths::new(&root);

        assert_eq!(paths.root(), root.join(".agy"));
        assert_eq!(paths.state_dir(), root.join(".agy/state"));
        assert_eq!(paths.workspaces_dir(), root.join(".agy/workspaces"));
        assert_eq!(paths.agent_workspace("coder"), root.join(".agy/workspaces/coder"));
        assert_eq!(paths.artifacts_dir(), root.join(".agy/artifacts"));
    }

    #[test]
    #[allow(clippy::expect_used, reason = "Unit test assertions")]
    fn test_legacy_pipeline_fallback() {
        let temp_dir = std::env::temp_dir().join(format!("agy_paths_test_{}", std::process::id()));
        let pipeline_dir = temp_dir.join(".pipeline");
        std::fs::create_dir_all(&pipeline_dir).expect("create legacy pipeline dir");

        // Write legacy circuit breaker
        let legacy_cb = pipeline_dir.join("circuit_breaker.json");
        std::fs::write(&legacy_cb, "{}").expect("write legacy cb");

        let paths = AgyStatePaths::new(&temp_dir);
        assert_eq!(paths.circuit_breaker_file(), legacy_cb);

        // Once modern file exists, it takes precedence
        let modern_state = paths.state_dir();
        std::fs::create_dir_all(&modern_state).expect("create modern state dir");
        let modern_cb = modern_state.join("circuit_breaker.json");
        std::fs::write(&modern_cb, "{}").expect("write modern cb");

        assert_eq!(paths.circuit_breaker_file(), modern_cb);

        drop(std::fs::remove_dir_all(&temp_dir));
    }
}
