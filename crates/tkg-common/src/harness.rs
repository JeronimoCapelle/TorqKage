//! # harness
//!
//! The [`Harness`] trait is the single seam between Torqkage and an agent execution harness
//! (Antigravity, `OpenCode`, Claude Code, or any custom CLI). Everything harness-specific that
//! the suite needs to know *statically* — binary name, CLI flags, directory conventions,
//! environment variables, plugin install locations — lives in an implementation of this trait.
//!
//! Adding a new harness means: implement [`Harness`], register it in [`resolve`] (and
//! [`builtin_harnesses`]), and optionally implement the behavioural `HarnessDriver` trait in
//! `tkg-ctrl` and the hook dialect in `tkg-hooks`.
//!
//! This module also hosts harness-neutral configuration discovery used by every binary.

use std::path::{Path, PathBuf};

use crate::project::{ProjectConfig, ProjectRunnerConfig};

/// Plugin bundle directory name used under every harness plugin root.
pub const BUNDLE_NAME: &str = "torqkage";

/// Static description of an agent execution harness.
pub trait Harness: Send + Sync + std::fmt::Debug {
    /// Canonical lowercase identifier (`agy`, `opencode`, `claude`, or the custom slug).
    fn id(&self) -> &str;

    /// Default executable name.
    fn default_binary(&self) -> String {
        self.id().to_owned()
    }

    /// Default CLI flag used to select an agent persona.
    fn agent_flag(&self) -> Option<&'static str> {
        Some("--agent")
    }

    /// Default CLI flag used to enable the harness' own sandbox.
    fn sandbox_flag(&self) -> Option<&'static str> {
        None
    }

    /// Default CLI flag used to resume a prior session.
    fn resume_flag(&self) -> Option<&'static str> {
        Some("--resume")
    }

    /// Harness-specific workspace directory name (e.g. `.claude`).
    fn workspace_dir(&self) -> Option<String>;

    /// User-global plugin root that contains the `torqkage` bundle directory.
    fn plugin_root(&self, home: &Path) -> Option<PathBuf>;

    /// Workspace-relative directories searched for agent manifests.
    fn workspace_agent_dirs(&self, workspace_root: &Path) -> Vec<PathBuf> {
        self.workspace_dir().map_or_default(|d| {
            vec![
                workspace_root.join(&d).join("agents"),
                workspace_root.join(d),
            ]
        })
    }

    /// User-global directories searched for agent manifests.
    fn home_agent_dirs(&self, home: &Path) -> Vec<PathBuf>;

    /// Environment variables through which this harness exposes the active agent name.
    fn agent_env_vars(&self) -> &'static [&'static str] {
        &[]
    }

    /// Environment variables set when the harness already runs inside its own sandbox.
    fn outer_sandbox_env_vars(&self) -> &'static [&'static str] {
        &[]
    }

    /// Path fragments identifying a plugin installation directory of this harness.
    fn plugin_path_markers(&self) -> Vec<String> {
        Vec::new()
    }

    /// Optional CLI command (argv) to validate a plugin bundle directory for this harness.
    fn validate_plugin_command(&self, _bundle_dir: &Path) -> Option<Vec<String>> {
        None
    }

    /// Whether agent manifests should emit Antigravity-specific YAML frontmatter keys
    /// (`permissionMode`, `commandExecutionPolicy`, `mainAgent`, `subagent`, `inheritMcp`, inline `mcpServers`).
    fn uses_agy_frontmatter(&self) -> bool {
        false
    }
}

/// Google Antigravity (`agy`).
#[derive(Debug, Default, Clone, Copy)]
pub struct Antigravity;

impl Harness for Antigravity {
    fn id(&self) -> &'static str {
        "agy"
    }
    fn sandbox_flag(&self) -> Option<&'static str> {
        Some("--sandbox")
    }
    fn resume_flag(&self) -> Option<&'static str> {
        Some("--conversation")
    }
    fn workspace_dir(&self) -> Option<String> {
        Some(".gemini".to_owned())
    }
    fn plugin_root(&self, home: &Path) -> Option<PathBuf> {
        Some(home.join(".gemini/config/plugins"))
    }
    fn workspace_agent_dirs(&self, workspace_root: &Path) -> Vec<PathBuf> {
        vec![workspace_root.join(".gemini/agents")]
    }
    fn home_agent_dirs(&self, home: &Path) -> Vec<PathBuf> {
        vec![
            home.join(".gemini/config/plugins")
                .join(BUNDLE_NAME)
                .join("agents"),
            home.join(".gemini/agents"),
        ]
    }
    fn agent_env_vars(&self) -> &'static [&'static str] {
        &["AGY_AGENT"]
    }
    fn outer_sandbox_env_vars(&self) -> &'static [&'static str] {
        &["AGY_OUTER_SANDBOX"]
    }
    fn plugin_path_markers(&self) -> Vec<String> {
        vec![
            "/.gemini/config/plugins/".to_owned(),
            "/.antigravity/plugins/".to_owned(),
            "/.gemini/plugins/".to_owned(),
        ]
    }
    fn validate_plugin_command(&self, bundle_dir: &Path) -> Option<Vec<String>> {
        Some(vec![
            "agy".to_owned(),
            "plugin".to_owned(),
            "validate".to_owned(),
            bundle_dir.to_string_lossy().into_owned(),
        ])
    }
    fn uses_agy_frontmatter(&self) -> bool {
        true
    }
}

/// `OpenCode`.
#[derive(Debug, Default, Clone, Copy)]
pub struct OpenCode;

impl Harness for OpenCode {
    fn id(&self) -> &'static str {
        "opencode"
    }
    fn resume_flag(&self) -> Option<&'static str> {
        Some("--session")
    }
    fn workspace_dir(&self) -> Option<String> {
        Some(".opencode".to_owned())
    }
    fn plugin_root(&self, home: &Path) -> Option<PathBuf> {
        Some(home.join(".local/share/opencode/plugins"))
    }
    fn home_agent_dirs(&self, home: &Path) -> Vec<PathBuf> {
        vec![
            home.join(".local/share/opencode/agents"),
            home.join(".opencode/agents"),
        ]
    }
    fn plugin_path_markers(&self) -> Vec<String> {
        vec![
            "/.opencode/plugins/".to_owned(),
            "/.local/share/opencode/plugins/".to_owned(),
        ]
    }
}

/// Claude Code.
#[derive(Debug, Default, Clone, Copy)]
pub struct ClaudeCode;

impl Harness for ClaudeCode {
    fn id(&self) -> &'static str {
        "claude"
    }
    fn workspace_dir(&self) -> Option<String> {
        Some(".claude".to_owned())
    }
    fn plugin_root(&self, home: &Path) -> Option<PathBuf> {
        Some(home.join(".claude/plugins"))
    }
    fn home_agent_dirs(&self, home: &Path) -> Vec<PathBuf> {
        vec![home.join(".claude/agents")]
    }
    fn plugin_path_markers(&self) -> Vec<String> {
        vec!["/.claude/plugins/".to_owned()]
    }
}

/// Any other CLI harness, addressed purely by convention (`.<slug>/agents`, `~/.<slug>/plugins`).
#[derive(Debug, Clone)]
pub struct Generic {
    slug: String,
}

impl Generic {
    /// Builds a generic harness from a user-provided name (sanitized to a filesystem-safe slug).
    #[must_use]
    pub fn new(name: &str) -> Self {
        let slug: String = name
            .trim()
            .to_ascii_lowercase()
            .chars()
            .filter(|c| c.is_ascii_alphanumeric() || *c == '-' || *c == '_')
            .collect();
        Self {
            slug: if slug.is_empty() {
                "custom".to_owned()
            } else {
                slug
            },
        }
    }

    fn is_placeholder(&self) -> bool {
        self.slug == "custom"
    }
}

impl Harness for Generic {
    fn id(&self) -> &str {
        &self.slug
    }
    fn workspace_dir(&self) -> Option<String> {
        (!self.is_placeholder()).then(|| format!(".{}", self.slug))
    }
    fn plugin_root(&self, home: &Path) -> Option<PathBuf> {
        Some(if self.is_placeholder() {
            home.join(".config/tkg/plugins")
        } else {
            home.join(format!(".{}/plugins", self.slug))
        })
    }
    fn home_agent_dirs(&self, home: &Path) -> Vec<PathBuf> {
        if self.is_placeholder() {
            Vec::new()
        } else {
            vec![home.join(format!(".{}/agents", self.slug))]
        }
    }
}

/// Resolves a harness implementation from its configured name (`[runner] harness = "..."`).
///
/// # Adding a New Harness
/// 1. Implement [`Harness`] for the new harness struct in this module.
/// 2. Add a match arm here and in [`builtin_harnesses`].
/// 3. Implement `HarnessDriver` in `tkg-ctrl` (`crates/tkg-ctrl/src/installer.rs`) and
///    `HookDialect` in `tkg-hooks` (`crates/tkg-hooks/src/stdin.rs`) if the harness uses custom
///    config registration, session storage, or hook wire JSON.
// TODO(harness): Add native implementations for Cursor, Windsurf, Aider, and Codex CLI.
#[must_use]
pub fn resolve(name: &str) -> Box<dyn Harness> {
    match name.trim().to_ascii_lowercase().as_str() {
        "agy" | "antigravity" => Box::new(Antigravity),
        "opencode" => Box::new(OpenCode),
        "claude" | "claude-code" => Box::new(ClaudeCode),
        other => Box::new(Generic::new(other)),
    }
}

/// All built-in harnesses (used for cross-harness fallbacks and marker detection).
#[must_use]
pub fn builtin_harnesses() -> Vec<Box<dyn Harness>> {
    vec![
        Box::new(Antigravity),
        Box::new(OpenCode),
        Box::new(ClaudeCode),
    ]
}

/// Returns the user-global install directory of the plugin bundle for the configured harness.
#[must_use]
pub fn plugin_install_dir(runner: &ProjectRunnerConfig) -> Option<PathBuf> {
    let home = home_dir()?;
    runner
        .harness()
        .plugin_root(&home)
        .map(|root| root.join(BUNDLE_NAME))
}

/// Ordered candidate paths for a declarative config file (highest priority first).
///
/// 1. `<root>/.tkg/<name>`
/// 2. `<root>/<name>`
/// 3. `<root>/plugins/torqkage/<name>`
/// 4. `<root>/<harness dir>/<name>`
/// 5. `~/.tkg/<name>` and `~/.config/tkg/<name>`
/// 6. configured harness plugin bundle, then every built-in harness bundle
#[must_use]
pub fn config_file_candidates(
    workspace_root: &Path,
    runner: &ProjectRunnerConfig,
    name: &str,
) -> Vec<PathBuf> {
    let harness = runner.harness();
    let mut out = vec![
        workspace_root.join(".tkg").join(name),
        workspace_root.join(name),
        workspace_root.join("plugins").join(BUNDLE_NAME).join(name),
    ];
    if let Some(dir) = harness.workspace_dir() {
        out.push(workspace_root.join(dir).join(name));
    }

    if let Some(home) = home_dir() {
        out.push(home.join(".tkg").join(name));
        out.push(home.join(".config/tkg").join(name));

        let builtins = builtin_harnesses();
        let roots = std::iter::once(harness.plugin_root(&home))
            .chain(builtins.iter().map(|h| h.plugin_root(&home)))
            .flatten();
        for root in roots {
            let p = root.join(BUNDLE_NAME).join(name);
            if !out.contains(&p) {
                out.push(p);
            }
        }
    }
    out
}

/// Loads the effective runner configuration of a workspace.
#[must_use]
pub fn runner_for(workspace_root: &Path) -> ProjectRunnerConfig {
    ProjectConfig::detect_or_load(workspace_root).resolved_runner()
}

/// Finds the first existing candidate for a declarative config file.
#[must_use]
pub fn find_config_file(workspace_root: &Path, name: &str) -> Option<PathBuf> {
    let runner = runner_for(workspace_root);
    config_file_candidates(workspace_root, &runner, name)
        .into_iter()
        .find(|p| p.is_file())
}

/// Finds the effective `tools.toml`.
#[must_use]
pub fn find_tools_toml(workspace_root: &Path) -> Option<PathBuf> {
    find_config_file(workspace_root, "tools.toml")
}

/// Finds the effective `hooks.toml`.
#[must_use]
pub fn find_hooks_toml(workspace_root: &Path) -> Option<PathBuf> {
    find_config_file(workspace_root, "hooks.toml")
}

fn home_dir() -> Option<PathBuf> {
    std::env::var_os("HOME").map(PathBuf::from)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn runner(h: &str) -> ProjectRunnerConfig {
        ProjectRunnerConfig {
            harness: h.to_owned(),
            ..ProjectRunnerConfig::default()
        }
    }

    #[test]
    fn test_resolve_and_defaults() {
        assert_eq!(resolve("Antigravity").id(), "agy");
        assert_eq!(resolve("claude-code").id(), "claude");
        assert_eq!(resolve("cursor").id(), "cursor");
        assert_eq!(resolve("cursor").default_binary(), "cursor");
        assert_eq!(resolve("agy").sandbox_flag(), Some("--sandbox"));
        assert_eq!(resolve("opencode").resume_flag(), Some("--session"));
        assert_eq!(resolve("whatever").resume_flag(), Some("--resume"));
    }

    #[test]
    fn test_workspace_harness_dir() {
        assert_eq!(
            runner("claude").harness().workspace_dir().as_deref(),
            Some(".claude")
        );
        assert_eq!(
            runner("agy").harness().workspace_dir().as_deref(),
            Some(".gemini")
        );
        assert_eq!(
            runner("cursor").harness().workspace_dir().as_deref(),
            Some(".cursor")
        );
        assert_eq!(runner("custom").harness().workspace_dir(), None);
    }

    #[test]
    fn test_candidates_priority_order() {
        let root = Path::new("/ws");
        let c = config_file_candidates(root, &runner("opencode"), "tools.toml");
        let expected = [
            "/ws/.tkg/tools.toml",
            "/ws/tools.toml",
            "/ws/plugins/torqkage/tools.toml",
            "/ws/.opencode/tools.toml",
        ];
        for (got, want) in c.iter().zip(expected) {
            assert_eq!(got, &PathBuf::from(want));
        }
    }
}
