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
    pub const ACTIVE_AGENT_FILE: &'static str = "active_agent.json";
    pub const ARTIFACTS_DIR: &'static str = "artifacts";
    pub const CACHE_DIR: &'static str = "cache";
    pub const CHECKPOINT_FILE: &'static str = "checkpoint.json";
    pub const CIRCUIT_BREAKER_FILE: &'static str = "circuit_breaker.json";
    pub const HANDOFF_JSON_FILE: &'static str = "handoff.json";
    pub const HANDOFF_MD_FILE: &'static str = "last_handoff.md";
    pub const ROOT_DIR: &'static str = ".agy";
    pub const STATE_DIR: &'static str = "state";
    pub const WORKSPACES_DIR: &'static str = "workspaces";

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

    /// The `.agy/cache` directory for transient ephemeral caches (WIP diffs, hashes).
    #[must_use]
    pub fn cache_dir(&self) -> PathBuf {
        self.root().join(Self::CACHE_DIR)
    }

    /// Path to the WIP change hash cache file for a specific conversation session.
    #[must_use]
    pub fn wip_cache_file(&self, conversation_id: Option<&str>) -> PathBuf {
        let filename = conversation_id.map_or_else(
            || ".wip_default.hash".to_owned(),
            |id| {
                let sanitized: String = id
                    .chars()
                    .filter(|c| c.is_alphanumeric() || *c == '-' || *c == '_')
                    .collect();
                format!(".wip_{sanitized}.hash")
            },
        );
        self.cache_dir().join("wip").join(filename)
    }

    /// Resolves the circuit breaker ledger file path (`.agy/state/circuit_breaker.json`).
    #[must_use]
    pub fn circuit_breaker_file(&self) -> PathBuf {
        self.state_dir().join(Self::CIRCUIT_BREAKER_FILE)
    }

    /// Resolves the structured handoff JSON file path (`.agy/state/handoff.json`).
    #[must_use]
    pub fn handoff_json_file(&self) -> PathBuf {
        self.state_dir().join(Self::HANDOFF_JSON_FILE)
    }

    /// Resolves the human-readable handoff Markdown file path (`.agy/state/last_handoff.md`).
    #[must_use]
    pub fn handoff_md_file(&self) -> PathBuf {
        self.state_dir().join(Self::HANDOFF_MD_FILE)
    }

    /// Resolves the active agent bridge file path (`.agy/state/active_agent.json`).
    #[must_use]
    pub fn active_agent_file(&self) -> PathBuf {
        self.state_dir().join(Self::ACTIVE_AGENT_FILE)
    }

    /// Resolves the session checkpoint file path (`.agy/artifacts/checkpoint.json`).
    #[must_use]
    pub fn checkpoint_file(&self) -> PathBuf {
        self.artifacts_dir().join(Self::CHECKPOINT_FILE)
    }

    /// Ensures that the core `.agy/` state and cache directories exist on disk.
    /// Automatically provisions a self-protecting `<workspace>/.agy/.gitignore` with `*`
    /// to guarantee external target Git repositories keep `.agy/` cleanly ignored.
    ///
    /// # Errors
    /// Returns `std::io::Error` if directory or `.gitignore` creation fails.
    pub fn ensure_directories(&self) -> std::io::Result<()> {
        std::fs::create_dir_all(self.state_dir())?;
        std::fs::create_dir_all(self.workspaces_dir())?;
        std::fs::create_dir_all(self.artifacts_dir())?;
        std::fs::create_dir_all(self.cache_dir().join("wip"))?;

        // Self-protecting gitignore: guarantees git ignores .agy in any project automatically
        let gitignore = self.root().join(".gitignore");
        if !gitignore.exists() {
            drop(std::fs::write(&gitignore, "*\n"));
        }
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
        assert_eq!(
            paths.agent_workspace("agent_worker"),
            root.join(".agy/workspaces/agent_worker")
        );
        assert_eq!(paths.artifacts_dir(), root.join(".agy/artifacts"));
        assert_eq!(paths.cache_dir(), root.join(".agy/cache"));
        assert_eq!(
            paths.circuit_breaker_file(),
            root.join(".agy/state/circuit_breaker.json")
        );
        assert_eq!(
            paths.handoff_json_file(),
            root.join(".agy/state/handoff.json")
        );
        assert_eq!(
            paths.handoff_md_file(),
            root.join(".agy/state/last_handoff.md")
        );
        assert_eq!(
            paths.active_agent_file(),
            root.join(".agy/state/active_agent.json")
        );
        assert_eq!(
            paths.checkpoint_file(),
            root.join(".agy/artifacts/checkpoint.json")
        );
    }

    #[test]
    #[allow(clippy::expect_used, reason = "Unit test assertions")]
    fn test_ensure_directories_creates_self_protecting_gitignore() {
        let temp_dir = std::env::temp_dir().join(format!("agy_paths_test_{}", std::process::id()));
        let paths = AgyStatePaths::new(&temp_dir);
        paths
            .ensure_directories()
            .expect("ensure directories failed");

        assert!(paths.state_dir().is_dir());
        assert!(paths.workspaces_dir().is_dir());
        assert!(paths.artifacts_dir().is_dir());
        assert!(paths.cache_dir().join("wip").is_dir());

        let gitignore = paths.root().join(".gitignore");
        assert!(gitignore.is_file());
        let content = std::fs::read_to_string(&gitignore).expect("read gitignore");
        assert_eq!(content, "*\n");

        drop(std::fs::remove_dir_all(paths.root()));
        drop(std::fs::remove_dir_all(&temp_dir));
    }
}
