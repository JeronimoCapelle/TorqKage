//! Declarative `hooks.toml` parser and hybrid configuration models.

use std::path::{Path, PathBuf};

use tkg_common::models::HookDecision;

/// Custom declarative check declared in `hooks.toml`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct CustomCheck {
    pub(crate) name: String,
    pub(crate) matcher: String,
    pub(crate) command: Vec<String>,
    pub(crate) fail_if_output_contains: Option<String>,
    pub(crate) decision: HookDecision,
    pub(crate) reason: String,
}

/// Pre-tool-use pipeline configuration.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct PreToolUseConfig {
    pub(crate) pipeline: Vec<String>,
    pub(crate) custom_checks: Vec<CustomCheck>,
}

impl Default for PreToolUseConfig {
    fn default() -> Self {
        Self {
            pipeline: vec![
                "circuit_breaker".to_owned(),
                "manifest_guard".to_owned(),
                "path_policies".to_owned(),
                "subagent_admission".to_owned(),
                "vcs_conflicts".to_owned(),
                "tool_policy".to_owned(),
            ],
            custom_checks: Vec::new(),
        }
    }
}

/// Circuit breaker configuration from `hooks.toml`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct CircuitBreakerConfig {
    pub(crate) enabled: bool,
    pub(crate) max_strikes: usize,
}

impl Default for CircuitBreakerConfig {
    fn default() -> Self {
        Self {
            enabled: true,
            max_strikes: tkg_common::CircuitBreakerLedger::DEFAULT_MAX_STRIKES,
        }
    }
}

/// Manifest mutation guard configuration protecting build files against host proc-macro execution escape.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct ManifestGuardConfig {
    pub(crate) enabled: bool,
    pub(crate) protected_patterns: Vec<String>,
    pub(crate) protected_tools: Vec<String>,
}

impl Default for ManifestGuardConfig {
    fn default() -> Self {
        Self {
            enabled: true,
            protected_patterns: vec![
                "Cargo.toml".into(),
                "Cargo.lock".into(),
                "build.rs".into(),
                "CMakeLists.txt".into(),
                "meson.build".into(),
                "package.json".into(),
                "build.zig".into(),
                "go.mod".into(),
                "Makefile".into(),
            ],
            protected_tools: Vec::new(),
        }
    }
}

/// Work-In-Progress (WIP) turn-by-turn change detector configuration.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct WipConfig {
    pub(crate) enabled: bool,
    pub(crate) max_preview_files: usize,
}

impl Default for WipConfig {
    fn default() -> Self {
        Self {
            enabled: true,
            max_preview_files: 4,
        }
    }
}

/// Declarative context injection for Turn 1 (`SessionStart`).
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct WakeInjection {
    pub(crate) name: String,
    pub(crate) matcher: String,
    pub(crate) command: Vec<String>,
    pub(crate) max_lines: usize,
    pub(crate) timeout_seconds: u64,
    pub(crate) prefix: String,
    pub(crate) fallback: Option<String>,
}

impl Default for WakeInjection {
    fn default() -> Self {
        Self {
            name: String::new(),
            matcher: "*".to_owned(),
            command: Vec::new(),
            max_lines: 40,
            timeout_seconds: 5,
            prefix: String::new(),
            fallback: None,
        }
    }
}

/// Session start context & wake-up injection configuration.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct SessionStartConfig {
    pub(crate) enabled: bool,
    pub(crate) inject_wake_state: bool,
    pub(crate) inject_documentation: bool,
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

/// Session stop handoff recording configuration.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct StopConfig {
    pub(crate) enabled: bool,
    pub(crate) record_handoff: bool,
    pub(crate) max_modified_files: usize,
}

impl Default for StopConfig {
    fn default() -> Self {
        Self {
            enabled: true,
            record_handoff: true,
            max_modified_files: 10,
        }
    }
}

/// Root configuration parsed from `hooks.toml`.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub(crate) struct HooksConfig {
    pub(crate) pre_tool_use: PreToolUseConfig,
    pub(crate) circuit_breaker: CircuitBreakerConfig,
    pub(crate) manifest_guard: ManifestGuardConfig,
    pub(crate) wip: WipConfig,
    pub(crate) session_start: SessionStartConfig,
    pub(crate) stop: StopConfig,
}

impl HooksConfig {
    /// Loads `hooks.toml` using hierarchical discovery:
    /// 1. `<root>/.tkg/hooks.toml` (workspace-local custom override)
    /// 2. `<root>/hooks.toml` (per-repo custom override)
    /// 3. `<root>/plugins/torqkage/hooks.toml` (in-tree plugin default)
    /// 4. `~/.gemini/config/plugins/torqkage/hooks.toml` (installed plugin fallback)
    #[must_use]
    pub(crate) fn load(workspace_root: &Path) -> Self {
        let mut candidates = vec![
            workspace_root.join(".tkg/hooks.toml"),
            workspace_root.join("hooks.toml"),
            workspace_root.join("plugins/torqkage/hooks.toml"),
        ];

        if let Ok(home) = std::env::var("HOME") {
            candidates.push(PathBuf::from(home).join(".gemini/config/plugins/torqkage/hooks.toml"));
        }

        for path in candidates {
            if path.is_file()
                && let Ok(content) = std::fs::read_to_string(&path)
            {
                return parse_hooks_config(&content);
            }
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
        } else if text == part {
            return true;
        }
    }
    false
}

fn parse_custom_check(table: &toml::Table) -> CustomCheck {
    let name = table
        .get("name")
        .and_then(toml::Value::as_str)
        .unwrap_or("")
        .to_owned();
    let matcher = table
        .get("matcher")
        .and_then(toml::Value::as_str)
        .unwrap_or("*")
        .to_owned();
    let command = table
        .get("command")
        .and_then(toml::Value::as_array)
        .map_or_default(|arr| {
            arr.iter()
                .filter_map(toml::Value::as_str)
                .map(ToOwned::to_owned)
                .collect()
        });
    let fail_if_output_contains = table
        .get("fail_if_output_contains")
        .and_then(toml::Value::as_str)
        .map(ToOwned::to_owned);
    let decision = if table
        .get("decision")
        .and_then(toml::Value::as_str)
        .is_some_and(|d| d.eq_ignore_ascii_case("ask"))
    {
        HookDecision::Ask
    } else {
        HookDecision::Deny
    };
    let reason = table
        .get("reason")
        .and_then(toml::Value::as_str)
        .unwrap_or("Declarative check condition failed")
        .to_owned();

    CustomCheck {
        name,
        matcher,
        command,
        fail_if_output_contains,
        decision,
        reason,
    }
}

fn parse_wake_injection(table: &toml::Table) -> WakeInjection {
    let name = table
        .get("name")
        .and_then(toml::Value::as_str)
        .unwrap_or("")
        .to_owned();
    let matcher = table
        .get("matcher")
        .and_then(toml::Value::as_str)
        .unwrap_or("*")
        .to_owned();
    let command = table
        .get("command")
        .and_then(toml::Value::as_array)
        .map_or_default(|arr| {
            arr.iter()
                .filter_map(toml::Value::as_str)
                .map(ToOwned::to_owned)
                .collect()
        });
    let max_lines = table
        .get("max_lines")
        .and_then(toml::Value::as_integer)
        .and_then(|n| usize::try_from(n).ok())
        .unwrap_or(40);
    let timeout_seconds = table
        .get("timeout_seconds")
        .and_then(toml::Value::as_integer)
        .and_then(|n| u64::try_from(n).ok())
        .unwrap_or(5);
    let prefix = table
        .get("prefix")
        .and_then(toml::Value::as_str)
        .unwrap_or("")
        .to_owned();
    let fallback = table
        .get("fallback")
        .and_then(toml::Value::as_str)
        .map(ToOwned::to_owned);

    WakeInjection {
        name,
        matcher,
        command,
        max_lines,
        timeout_seconds,
        prefix,
        fallback,
    }
}

fn parse_pre_tool_use(pre: &toml::Table) -> PreToolUseConfig {
    let mut config = PreToolUseConfig::default();
    if let Some(pipeline) = pre.get("pipeline").and_then(toml::Value::as_array) {
        config.pipeline = pipeline
            .iter()
            .filter_map(toml::Value::as_str)
            .map(ToOwned::to_owned)
            .collect();
    }
    if let Some(checks) = pre.get("custom_checks").and_then(toml::Value::as_array) {
        for check in checks {
            if let Some(ct) = check.as_table() {
                config.custom_checks.push(parse_custom_check(ct));
            }
        }
    }
    config
}

fn parse_circuit_breaker(cb: &toml::Table) -> CircuitBreakerConfig {
    let mut config = CircuitBreakerConfig::default();
    if let Some(enabled) = cb.get("enabled").and_then(toml::Value::as_bool) {
        config.enabled = enabled;
    }
    if let Some(max) = cb.get("max_strikes").and_then(toml::Value::as_integer)
        && let Ok(m) = usize::try_from(max)
    {
        config.max_strikes = m;
    }
    config
}

fn parse_manifest_guard(mg: &toml::Table) -> ManifestGuardConfig {
    let mut config = ManifestGuardConfig::default();
    if let Some(enabled) = mg.get("enabled").and_then(toml::Value::as_bool) {
        config.enabled = enabled;
    }
    if let Some(patterns) = mg.get("protected_patterns").and_then(toml::Value::as_array) {
        config.protected_patterns = patterns
            .iter()
            .filter_map(toml::Value::as_str)
            .map(ToOwned::to_owned)
            .collect();
    }
    if let Some(tools) = mg.get("protected_tools").and_then(toml::Value::as_array) {
        config.protected_tools = tools
            .iter()
            .filter_map(toml::Value::as_str)
            .map(ToOwned::to_owned)
            .collect();
    }
    config
}

fn parse_wip(wip: &toml::Table) -> WipConfig {
    let mut config = WipConfig::default();
    if let Some(enabled) = wip.get("enabled").and_then(toml::Value::as_bool) {
        config.enabled = enabled;
    }
    if let Some(max) = wip
        .get("max_preview_files")
        .and_then(toml::Value::as_integer)
        && let Ok(m) = usize::try_from(max)
    {
        config.max_preview_files = m;
    }
    config
}

fn parse_session_start(ss: &toml::Table) -> SessionStartConfig {
    let mut config = SessionStartConfig::default();
    if let Some(enabled) = ss.get("enabled").and_then(toml::Value::as_bool) {
        config.enabled = enabled;
    }
    if let Some(wake) = ss.get("inject_wake_state").and_then(toml::Value::as_bool) {
        config.inject_wake_state = wake;
    }
    if let Some(doc) = ss
        .get("inject_documentation")
        .and_then(toml::Value::as_bool)
    {
        config.inject_documentation = doc;
    }
    let injections_arr = ss
        .get("injections")
        .or_else(|| ss.get("custom_injections"))
        .and_then(toml::Value::as_array);
    if let Some(injections) = injections_arr {
        for inj in injections {
            if let Some(it) = inj.as_table() {
                config.injections.push(parse_wake_injection(it));
            }
        }
    }
    config
}

fn parse_stop(stop: &toml::Table) -> StopConfig {
    let mut config = StopConfig::default();
    if let Some(enabled) = stop.get("enabled").and_then(toml::Value::as_bool) {
        config.enabled = enabled;
    }
    if let Some(record) = stop.get("record_handoff").and_then(toml::Value::as_bool) {
        config.record_handoff = record;
    }
    if let Some(max) = stop
        .get("max_modified_files")
        .and_then(toml::Value::as_integer)
        && let Ok(m) = usize::try_from(max)
    {
        config.max_modified_files = m;
    }
    config
}

/// Parses a `hooks.toml` string into a structured `HooksConfig`.
#[must_use]
pub(crate) fn parse_hooks_config(content: &str) -> HooksConfig {
    let Ok(table) = content.parse::<toml::Table>() else {
        return HooksConfig::default();
    };

    HooksConfig {
        pre_tool_use: table
            .get("pre_tool_use")
            .and_then(toml::Value::as_table)
            .map_or_default(parse_pre_tool_use),
        circuit_breaker: table
            .get("circuit_breaker")
            .and_then(toml::Value::as_table)
            .map_or_default(parse_circuit_breaker),
        manifest_guard: table
            .get("manifest_guard")
            .and_then(toml::Value::as_table)
            .map_or_default(parse_manifest_guard),
        wip: table
            .get("wip")
            .and_then(toml::Value::as_table)
            .map_or_default(parse_wip),
        session_start: table
            .get("session_start")
            .and_then(toml::Value::as_table)
            .map_or_default(parse_session_start),
        stop: table
            .get("stop")
            .and_then(toml::Value::as_table)
            .map_or_default(parse_stop),
    }
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
