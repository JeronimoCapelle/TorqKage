//! Centralized state and workspace path resolution for the `Torqkage` ecosystem.
//!
//! All runtime state, isolated workspaces, and inter-agent artifacts are
//! consolidated under the `.tkg/` root directory in the workspace root:
//! - `.tkg/state/`: Ephemeral runtime state (circuit breaker ledger, session handoffs, active agent bridges)
//! - `.tkg/workspaces/`: Isolated subagent Jujutsu worktrees (`.tkg/workspaces/<agent>`)
//! - `.tkg/artifacts/`: Checkpoints, performance profiles, and generated deliverables

use std::path::{Path, PathBuf};

/// Path resolver for the `.tkg/` state repository.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TkgStatePaths {
    workspace_root: PathBuf,
}

impl TkgStatePaths {
    pub const ACTIVE_AGENT_FILE: &'static str = "active_agent.json";
    pub const ARTIFACTS_DIR: &'static str = "artifacts";
    pub const CACHE_DIR: &'static str = "cache";
    pub const CHECKPOINT_FILE: &'static str = "checkpoint.json";
    pub const CIRCUIT_BREAKER_FILE: &'static str = "circuit_breaker.json";
    pub const HANDOFF_JSON_FILE: &'static str = "handoff.json";
    pub const HANDOFF_MD_FILE: &'static str = "last_handoff.md";
    pub const ROOT_DIR: &'static str = ".tkg";
    pub const STATE_DIR: &'static str = "state";
    pub const WORKSPACES_DIR: &'static str = "workspaces";

    /// Constructs a `TkgStatePaths` instance rooted at `workspace_root`.
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

    /// The base directory `.tkg` under the workspace root.
    #[must_use]
    pub fn root(&self) -> PathBuf {
        self.workspace_root.join(Self::ROOT_DIR)
    }

    /// The `.tkg/state` directory for ephemeral runtime state.
    #[must_use]
    pub fn state_dir(&self) -> PathBuf {
        self.root().join(Self::STATE_DIR)
    }

    /// The `.tkg/workspaces` directory for isolated subagent Jujutsu workspaces.
    #[must_use]
    pub fn workspaces_dir(&self) -> PathBuf {
        self.root().join(Self::WORKSPACES_DIR)
    }

    /// Path to a specific subagent's isolated workspace: `.tkg/workspaces/<agent>`.
    #[must_use]
    pub fn agent_workspace(&self, agent: &str) -> PathBuf {
        self.workspaces_dir().join(agent)
    }

    /// The `.tkg/artifacts` directory for durable generated deliverables.
    #[must_use]
    pub fn artifacts_dir(&self) -> PathBuf {
        self.root().join(Self::ARTIFACTS_DIR)
    }

    /// The `.tkg/cache` directory for transient ephemeral caches (WIP diffs, hashes).
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

    /// Resolves the circuit breaker ledger file path (`.tkg/state/circuit_breaker.json`).
    #[must_use]
    pub fn circuit_breaker_file(&self) -> PathBuf {
        self.state_dir().join(Self::CIRCUIT_BREAKER_FILE)
    }

    /// Resolves the structured handoff JSON file path (`.tkg/state/handoff.json`).
    #[must_use]
    pub fn handoff_json_file(&self) -> PathBuf {
        self.state_dir().join(Self::HANDOFF_JSON_FILE)
    }

    /// Resolves the human-readable handoff Markdown file path (`.tkg/state/last_handoff.md`).
    #[must_use]
    pub fn handoff_md_file(&self) -> PathBuf {
        self.state_dir().join(Self::HANDOFF_MD_FILE)
    }

    /// Resolves the active agent bridge file path (`.tkg/state/active_agent.json`).
    #[must_use]
    pub fn active_agent_file(&self) -> PathBuf {
        self.state_dir().join(Self::ACTIVE_AGENT_FILE)
    }

    /// Resolves the session checkpoint file path (`.tkg/artifacts/checkpoint.json`).
    #[must_use]
    pub fn checkpoint_file(&self) -> PathBuf {
        self.artifacts_dir().join(Self::CHECKPOINT_FILE)
    }

    /// Ensures that the core `.tkg/` state and cache directories exist on disk.
    /// Automatically provisions a self-protecting `<workspace>/.tkg/.gitignore` with `*`
    /// to guarantee external target Git repositories keep `.tkg/` cleanly ignored.
    ///
    /// # Errors
    /// Returns `std::io::Error` if directory or `.gitignore` creation fails.
    pub fn ensure_directories(&self) -> std::io::Result<()> {
        std::fs::create_dir_all(self.state_dir())?;
        std::fs::create_dir_all(self.workspaces_dir())?;
        std::fs::create_dir_all(self.artifacts_dir())?;
        std::fs::create_dir_all(self.cache_dir().join("wip"))?;

        // Self-protecting gitignore: guarantees git ignores .tkg in any project automatically
        let gitignore = self.root().join(".gitignore");
        if !gitignore.exists() {
            drop(std::fs::write(&gitignore, "*\n"));
        }
        Ok(())
    }
}

/// Resolves the currently active agent name using hierarchical discovery:
/// 1. `TKG_AGENT` environment variable
/// 2. harness-specific environment variables (see [`crate::Harness::agent_env_vars`])
/// 3. `AGENT` environment variable
/// 4. `MCP_SCOPE` environment variable
/// 5. `.tkg/state/active_agent.json` (`{"agent": "..."}`)
/// 6. `.tkg/state/active_agent` (plain text)
#[must_use]
pub fn resolve_active_agent(workspace_root: &Path) -> Option<String> {
    let harness = crate::harness::runner_for(workspace_root).harness();
    let env_vars = std::iter::once("TKG_AGENT")
        .chain(harness.agent_env_vars().iter().copied())
        .chain(["AGENT", "MCP_SCOPE"]);
    for var in env_vars {
        if let Ok(val) = std::env::var(var) {
            let trimmed = val.trim();
            if !trimmed.is_empty() {
                return Some(trimmed.to_owned());
            }
        }
    }

    let paths = TkgStatePaths::new(workspace_root);
    let json_path = paths.active_agent_file();
    if json_path.is_file()
        && let Ok(content) = std::fs::read_to_string(&json_path)
        && let Ok(val) = serde_json::from_str::<serde_json::Value>(&content)
        && let Some(name) = val.get("agent").and_then(serde_json::Value::as_str)
    {
        let trimmed = name.trim();
        if !trimmed.is_empty() {
            return Some(trimmed.to_owned());
        }
    }

    let txt_path = paths.state_dir().join("active_agent");
    if txt_path.is_file()
        && let Ok(content) = std::fs::read_to_string(&txt_path)
    {
        let trimmed = content.trim();
        if !trimmed.is_empty() {
            return Some(trimmed.to_owned());
        }
    }

    None
}

/// Writes `content` to `path` atomically by creating a temporary file in the same parent directory,
/// writing all bytes, and performing an atomic POSIX `rename(2)` via [`tempfile::NamedTempFile::persist`].
///
/// # Errors
/// Returns `std::io::Error` if parent directory creation, temporary file write, or atomic rename fails.
pub fn atomic_write(path: &Path, content: &[u8]) -> std::io::Result<()> {
    use std::io::Write as _;

    let parent = path
        .parent()
        .filter(|p| !p.as_os_str().is_empty())
        .unwrap_or_else(|| Path::new("."));
    if !parent.exists() {
        std::fs::create_dir_all(parent)?;
    }

    let mut tmp = tempfile::NamedTempFile::new_in(parent)?;
    tmp.write_all(content)?;
    tmp.persist(path).map_err(|e| e.error)?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    #[allow(clippy::expect_used, reason = "Unit test assertions")]
    fn test_tkg_state_paths_structure() {
        let root = PathBuf::from("/home/user/project");
        let paths = TkgStatePaths::new(&root);

        assert_eq!(paths.root(), root.join(".tkg"));
        assert_eq!(paths.state_dir(), root.join(".tkg/state"));
        assert_eq!(paths.workspaces_dir(), root.join(".tkg/workspaces"));
        assert_eq!(
            paths.agent_workspace("agent_worker"),
            root.join(".tkg/workspaces/agent_worker")
        );
        assert_eq!(paths.artifacts_dir(), root.join(".tkg/artifacts"));
        assert_eq!(paths.cache_dir(), root.join(".tkg/cache"));
        assert_eq!(
            paths.circuit_breaker_file(),
            root.join(".tkg/state/circuit_breaker.json")
        );
        assert_eq!(
            paths.handoff_json_file(),
            root.join(".tkg/state/handoff.json")
        );
        assert_eq!(
            paths.handoff_md_file(),
            root.join(".tkg/state/last_handoff.md")
        );
        assert_eq!(
            paths.active_agent_file(),
            root.join(".tkg/state/active_agent.json")
        );
        assert_eq!(
            paths.checkpoint_file(),
            root.join(".tkg/artifacts/checkpoint.json")
        );
    }

    #[test]
    #[allow(clippy::expect_used, reason = "Unit test assertions")]
    fn test_ensure_directories_creates_self_protecting_gitignore() {
        let temp_dir = std::env::temp_dir().join(format!("tkg_paths_test_{}", std::process::id()));
        let paths = TkgStatePaths::new(&temp_dir);
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

    #[test]
    #[allow(clippy::unwrap_used, reason = "Unit test assertions")]
    fn test_resolve_active_agent_from_file() {
        let temp_dir =
            std::env::temp_dir().join(format!("tkg_test_resolve_agent_{}", std::process::id()));
        let paths = TkgStatePaths::new(&temp_dir);
        drop(paths.ensure_directories());

        let json = serde_json::json!({ "agent": "reviewer" });
        std::fs::write(
            paths.active_agent_file(),
            serde_json::to_string(&json).unwrap(),
        )
        .unwrap();

        assert_eq!(resolve_active_agent(&temp_dir), Some("reviewer".to_owned()));

        drop(std::fs::remove_dir_all(&temp_dir));
    }
}
