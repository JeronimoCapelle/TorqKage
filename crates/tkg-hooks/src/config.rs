//! Declarative `hooks.toml` parser and hybrid configuration models.

use std::path::Path;

use serde::Deserialize;
use tkg_common::models::HookDecision;

const fn default_true() -> bool {
    true
}

fn default_matcher() -> String {
    "*".to_owned()
}

fn default_custom_check_reason() -> String {
    "Declarative check condition failed".to_owned()
}

fn deserialize_custom_decision<'de, D>(deserializer: D) -> Result<HookDecision, D::Error>
where
    D: serde::Deserializer<'de>,
{
    let opt = Option::<String>::deserialize(deserializer)?;
    if opt.is_some_and(|d| d.eq_ignore_ascii_case("ask")) {
        Ok(HookDecision::Ask)
    } else {
        Ok(HookDecision::Deny)
    }
}

const fn default_deny() -> HookDecision {
    HookDecision::Deny
}

/// Custom declarative check declared in `hooks.toml`.
#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
pub(crate) struct CustomCheck {
    #[serde(default)]
    pub(crate) name: String,
    #[serde(default = "default_matcher")]
    pub(crate) matcher: String,
    #[serde(default)]
    pub(crate) command: Vec<String>,
    #[serde(default)]
    pub(crate) fail_if_output_contains: Option<String>,
    #[serde(
        default = "default_deny",
        deserialize_with = "deserialize_custom_decision"
    )]
    pub(crate) decision: HookDecision,
    #[serde(default = "default_custom_check_reason")]
    pub(crate) reason: String,
}

fn default_pipeline() -> Vec<String> {
    vec![
        "circuit_breaker".to_owned(),
        "manifest_guard".to_owned(),
        "path_policies".to_owned(),
        "subagent_admission".to_owned(),
        "vcs_conflicts".to_owned(),
        "tool_policy".to_owned(),
    ]
}

/// Pre-tool-use pipeline configuration.
#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
pub(crate) struct PreToolUseConfig {
    #[serde(default = "default_pipeline")]
    pub(crate) pipeline: Vec<String>,
    #[serde(default)]
    pub(crate) custom_checks: Vec<CustomCheck>,
}

impl Default for PreToolUseConfig {
    fn default() -> Self {
        Self {
            pipeline: default_pipeline(),
            custom_checks: Vec::new(),
        }
    }
}

const fn default_max_strikes() -> usize {
    tkg_common::CircuitBreakerLedger::DEFAULT_MAX_STRIKES
}

/// Circuit breaker configuration from `hooks.toml`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize)]
pub(crate) struct CircuitBreakerConfig {
    #[serde(default = "default_true")]
    pub(crate) enabled: bool,
    #[serde(default = "default_max_strikes")]
    pub(crate) max_strikes: usize,
}

impl Default for CircuitBreakerConfig {
    fn default() -> Self {
        Self {
            enabled: true,
            max_strikes: default_max_strikes(),
        }
    }
}

fn default_protected_patterns() -> Vec<String> {
    vec![
        "Cargo.toml".into(),
        "Cargo.lock".into(),
        "build.rs".into(),
        "CMakeLists.txt".into(),
        "meson.build".into(),
        "package.json".into(),
        "build.zig".into(),
        "go.mod".into(),
        "Makefile".into(),
    ]
}

/// Manifest mutation guard configuration protecting build files against host proc-macro execution escape.
#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
pub(crate) struct ManifestGuardConfig {
    #[serde(default = "default_true")]
    pub(crate) enabled: bool,
    #[serde(default = "default_protected_patterns")]
    pub(crate) protected_patterns: Vec<String>,
    #[serde(default)]
    pub(crate) protected_tools: Vec<String>,
}

impl Default for ManifestGuardConfig {
    fn default() -> Self {
        Self {
            enabled: true,
            protected_patterns: default_protected_patterns(),
            protected_tools: Vec::new(),
        }
    }
}

const fn default_max_preview_files() -> usize {
    4
}

/// Work-In-Progress (WIP) turn-by-turn change detector configuration.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize)]
pub(crate) struct WipConfig {
    #[serde(default = "default_true")]
    pub(crate) enabled: bool,
    #[serde(default = "default_max_preview_files")]
    pub(crate) max_preview_files: usize,
}

impl Default for WipConfig {
    fn default() -> Self {
        Self {
            enabled: true,
            max_preview_files: default_max_preview_files(),
        }
    }
}

const fn default_max_lines() -> usize {
    40
}

const fn default_wake_timeout_seconds() -> u64 {
    5
}

/// Declarative context injection for Turn 1 (`SessionStart`).
#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
pub(crate) struct WakeInjection {
    #[serde(default)]
    pub(crate) name: String,
    #[serde(default = "default_matcher")]
    pub(crate) matcher: String,
    #[serde(default)]
    pub(crate) command: Vec<String>,
    #[serde(default = "default_max_lines")]
    pub(crate) max_lines: usize,
    #[serde(default = "default_wake_timeout_seconds")]
    pub(crate) timeout_seconds: u64,
    #[serde(default)]
    pub(crate) prefix: String,
    #[serde(default)]
    pub(crate) fallback: Option<String>,
}

impl Default for WakeInjection {
    fn default() -> Self {
        Self {
            name: String::new(),
            matcher: default_matcher(),
            command: Vec::new(),
            max_lines: default_max_lines(),
            timeout_seconds: default_wake_timeout_seconds(),
            prefix: String::new(),
            fallback: None,
        }
    }
}

/// Session start context & wake-up injection configuration.
#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
pub(crate) struct SessionStartConfig {
    #[serde(default = "default_true")]
    pub(crate) enabled: bool,
    #[serde(default = "default_true")]
    pub(crate) inject_wake_state: bool,
    #[serde(default = "default_true")]
    pub(crate) inject_documentation: bool,
    #[serde(default, alias = "custom_injections")]
    pub(crate) injections: Vec<WakeInjection>,
}

impl Default for SessionStartConfig {
    fn default() -> Self {
        Self {
            enabled: true,
            inject_wake_state: true,
            inject_documentation: true,
            injections: Vec::new(),
        }
    }
}

const fn default_max_modified_files() -> usize {
    10
}

/// Session stop handoff recording configuration.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize)]
pub(crate) struct StopConfig {
    #[serde(default = "default_true")]
    pub(crate) enabled: bool,
    #[serde(default = "default_true")]
    pub(crate) record_handoff: bool,
    #[serde(default = "default_max_modified_files")]
    pub(crate) max_modified_files: usize,
}

impl Default for StopConfig {
    fn default() -> Self {
        Self {
            enabled: true,
            record_handoff: true,
            max_modified_files: default_max_modified_files(),
        }
    }
}

/// Root configuration parsed from `hooks.toml`.
#[derive(Debug, Clone, PartialEq, Eq, Default, Deserialize)]
pub(crate) struct HooksConfig {
    #[serde(default)]
    pub(crate) pre_tool_use: PreToolUseConfig,
    #[serde(default)]
    pub(crate) circuit_breaker: CircuitBreakerConfig,
    #[serde(default)]
    pub(crate) manifest_guard: ManifestGuardConfig,
    #[serde(default)]
    pub(crate) wip: WipConfig,
    #[serde(default)]
    pub(crate) session_start: SessionStartConfig,
    #[serde(default)]
    pub(crate) stop: StopConfig,
}

impl HooksConfig {
    /// Loads `hooks.toml` using harness-neutral hierarchical discovery
    /// (see [`tkg_common::harness::config_file_candidates`]).
    #[must_use]
    pub(crate) fn load(workspace_root: &Path) -> Self {
        if let Some(path) = tkg_common::harness::find_hooks_toml(workspace_root)
            && let Ok(content) = std::fs::read_to_string(&path)
        {
            return parse_hooks_config(&content);
        }

        Self::default()
    }
}

/// Matches a string against a pattern supporting wildcards (`*`), prefixes (`foo_.*`, `foo_*`),
/// or pipe-separated choices (`a|b|c`).
#[must_use]
pub(crate) fn matches_pattern(text: &str, pattern: &str) -> bool {
    let p = pattern.trim();
    if p == "*" || p.is_empty() {
        return true;
    }
    for part in p.split('|') {
        let part = part.trim();
        if part == "*" {
            return true;
        }
        if let Some(prefix) = part.strip_suffix(".*").or_else(|| part.strip_suffix('*')) {
            if text.starts_with(prefix) {
                return true;
            }
        } else if tkg_common::path_matches_pattern(text, part) {
            return true;
        }
    }
    false
}

/// Parses a `hooks.toml` string into a structured `HooksConfig`.
#[must_use]
pub(crate) fn parse_hooks_config(content: &str) -> HooksConfig {
    toml::from_str(content).unwrap_or_default()
}

#[cfg(test)]
#[allow(
    clippy::unwrap_used,
    clippy::indexing_slicing,
    reason = "Unit test assertions and indexing"
)]
mod tests {
    use super::*;

    #[test]
    fn test_matches_pattern_wildcard() {
        assert!(matches_pattern("anything", "*"));
        assert!(matches_pattern("anything", ""));
        assert!(matches_pattern("cargo_check", "cargo_.*|quick_check"));
        assert!(matches_pattern("quick_check", "cargo_.*|quick_check"));
        assert!(!matches_pattern("modify_code", "cargo_.*|quick_check"));
    }

    #[test]
    fn test_parse_hooks_config_full() {
        let sample = r#"
[pre_tool_use]
pipeline = [
    "circuit_breaker",
    "subagent_admission",
    "vcs_conflicts",
    "tool_policy",
]

[[pre_tool_use.custom_checks]]
name = "check_disk"
matcher = "modify_code|write_file"
command = ["df", "-h", "."]
fail_if_output_contains = "100%"
decision = "deny"
reason = "Disk is full"
"#;

        let parsed = parse_hooks_config(sample);
        assert_eq!(parsed.pre_tool_use.pipeline.len(), 4);
        assert_eq!(parsed.pre_tool_use.pipeline[0], "circuit_breaker");
        assert_eq!(parsed.pre_tool_use.custom_checks.len(), 1);
        assert_eq!(parsed.pre_tool_use.custom_checks[0].name, "check_disk");
        assert_eq!(
            parsed.pre_tool_use.custom_checks[0].matcher,
            "modify_code|write_file"
        );
        assert_eq!(
            parsed.pre_tool_use.custom_checks[0].command,
            vec!["df", "-h", "."]
        );
        assert_eq!(
            parsed.pre_tool_use.custom_checks[0]
                .fail_if_output_contains
                .as_deref(),
            Some("100%")
        );
        assert_eq!(
            parsed.pre_tool_use.custom_checks[0].decision,
            HookDecision::Deny
        );
    }
}
