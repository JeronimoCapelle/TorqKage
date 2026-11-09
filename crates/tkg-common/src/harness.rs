//! # harness
//!
//! Harness-neutral configuration discovery. Every binary in the suite (`tkg`, `tkg-mcp`,
//! `tkg-hooks`) resolves declarative files (`tools.toml`, `hooks.toml`) and plugin install
//! locations through this single module, driven by the `[runner] harness = "..."` selection
//! in `project.toml`. Supporting a new harness only requires extending the functions below.

use std::path::{Path, PathBuf};

use crate::project::{HarnessKind, ProjectConfig, ProjectRunnerConfig};

/// Plugin bundle directory name used under every harness plugin root.
pub const BUNDLE_NAME: &str = "torqkage";

/// Returns the harness-specific workspace directory (e.g. `.claude`, `.opencode`, `.gemini`).
#[must_use]
pub fn workspace_harness_dir(runner: &ProjectRunnerConfig) -> Option<String> {
    match runner.harness_kind() {
        HarnessKind::Agy => Some(".gemini".to_owned()),
        HarnessKind::OpenCode => Some(".opencode".to_owned()),
        HarnessKind::Claude => Some(".claude".to_owned()),
        HarnessKind::Custom => custom_slug(runner).map(|s| format!(".{s}")),
    }
}

/// Returns the user-global plugin root directory of the harness (the directory that
/// contains the `torqkage` bundle directory).
#[must_use]
pub fn plugin_root_for(kind: HarnessKind, slug: Option<&str>, home: &Path) -> Option<PathBuf> {
    match kind {
        HarnessKind::Agy => Some(home.join(".gemini/config/plugins")),
        HarnessKind::OpenCode => Some(home.join(".local/share/opencode/plugins")),
        HarnessKind::Claude => Some(home.join(".claude/plugins")),
        HarnessKind::Custom => slug.map(|s| home.join(format!(".{s}/plugins"))),
    }
}

/// Returns the user-global install directory of the plugin bundle for the configured harness.
#[must_use]
pub fn plugin_install_dir(runner: &ProjectRunnerConfig) -> Option<PathBuf> {
    let home = home_dir()?;
    plugin_root_for(runner.harness_kind(), custom_slug(runner).as_deref(), &home)
        .map(|root| root.join(BUNDLE_NAME))
}

/// Ordered candidate paths for a declarative config file (highest priority first).
///
/// 1. `<root>/.tkg/<name>`
/// 2. `<root>/<name>`
/// 3. `<root>/plugins/torqkage/<name>`
/// 4. `<root>/<harness dir>/<name>`
/// 5. `~/.tkg/<name>` and `~/.config/tkg/<name>`
/// 6. configured harness plugin bundle, then every other known harness bundle
#[must_use]
pub fn config_file_candidates(
    workspace_root: &Path,
    runner: &ProjectRunnerConfig,
    name: &str,
) -> Vec<PathBuf> {
    let mut out = vec![
        workspace_root.join(".tkg").join(name),
        workspace_root.join(name),
        workspace_root.join("plugins").join(BUNDLE_NAME).join(name),
    ];
    if let Some(dir) = workspace_harness_dir(runner) {
        out.push(workspace_root.join(dir).join(name));
    }

    if let Some(home) = home_dir() {
        out.push(home.join(".tkg").join(name));
        out.push(home.join(".config/tkg").join(name));

        let slug = custom_slug(runner);
        if let Some(root) = plugin_root_for(runner.harness_kind(), slug.as_deref(), &home) {
            out.push(root.join(BUNDLE_NAME).join(name));
        }
        for kind in [HarnessKind::Agy, HarnessKind::OpenCode, HarnessKind::Claude] {
            if let Some(root) = plugin_root_for(kind, None, &home) {
                let p = root.join(BUNDLE_NAME).join(name);
                if !out.contains(&p) {
                    out.push(p);
                }
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

/// Filesystem-safe slug of a custom harness name (`None` for the generic `custom` placeholder).
fn custom_slug(runner: &ProjectRunnerConfig) -> Option<String> {
    let slug: String = runner
        .harness
        .trim()
        .to_ascii_lowercase()
        .chars()
        .filter(|c| c.is_ascii_alphanumeric() || *c == '-' || *c == '_')
        .collect();
    (!slug.is_empty() && slug != "custom").then_some(slug)
}

/// Public accessor for the custom harness slug used in convention-based directories.
#[must_use]
pub fn harness_slug(runner: &ProjectRunnerConfig) -> Option<String> {
    custom_slug(runner)
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
    fn test_workspace_harness_dir() {
        assert_eq!(
            workspace_harness_dir(&runner("claude")).as_deref(),
            Some(".claude")
        );
        assert_eq!(
            workspace_harness_dir(&runner("agy")).as_deref(),
            Some(".gemini")
        );
        assert_eq!(
            workspace_harness_dir(&runner("cursor")).as_deref(),
            Some(".cursor")
        );
        assert_eq!(workspace_harness_dir(&runner("custom")), None);
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
