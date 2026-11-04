//! Declarative `hooks.toml` parser and hybrid configuration models.

#![allow(clippy::string_slice, reason = "Line and character parser operates on ASCII delimiter offsets")]

use std::path::{Path, PathBuf};
use agy_common::models::HookDecision;

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
    pub(crate) vcs_conflicts_matcher: String,
    pub(crate) custom_checks: Vec<CustomCheck>,
}

impl Default for PreToolUseConfig {
    fn default() -> Self {
        Self {
            pipeline: vec![
                "circuit_breaker".to_owned(),
                "subagent_admission".to_owned(),
                "vcs_conflicts".to_owned(),
                "tool_policy".to_owned(),
            ],
            vcs_conflicts_matcher: ".*compile.*|.*build.*|.*test.*|.*check.*|run_cargo_.*|run_mutation_tests|run_coverage_summary|run_quick_check".to_owned(),
            custom_checks: Vec::new(),
        }
    }
}

/// Circuit breaker configuration from `hooks.toml`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct CircuitBreakerConfig {
    pub(crate) max_strikes: usize,
}

impl Default for CircuitBreakerConfig {
    fn default() -> Self {
        Self {
            max_strikes: agy_common::CircuitBreakerLedger::DEFAULT_MAX_STRIKES,
        }
    }
}

/// Root configuration parsed from `hooks.toml`.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub(crate) struct HooksConfig {
    pub(crate) pre_tool_use: PreToolUseConfig,
    pub(crate) circuit_breaker: CircuitBreakerConfig,
}

impl HooksConfig {
    /// Loads `hooks.toml` using hierarchical discovery:
    /// 1. `<root>/hooks.toml` (per-repo custom override)
    /// 2. `<root>/plugins/agy-suite/hooks.toml` (in-tree plugin default)
    /// 3. `~/.gemini/config/plugins/agy-suite/hooks.toml` (installed plugin fallback)
    #[must_use]
    pub(crate) fn load(workspace_root: &Path) -> Self {
        let mut candidates = vec![
            workspace_root.join("hooks.toml"),
            workspace_root.join("plugins/agy-suite/hooks.toml"),
        ];

        if let Ok(home) = std::env::var("HOME") {
            candidates.push(PathBuf::from(home).join(".gemini/config/plugins/agy-suite/hooks.toml"));
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

/// Parses a `hooks.toml` string into a structured `HooksConfig`.
#[must_use]
pub(crate) fn parse_hooks_config(content: &str) -> HooksConfig {
    let mut config = HooksConfig::default();
    let mut current_section = "";
    let mut current_check: Option<CustomCheck> = None;

    let mut in_array = false;
    let mut array_key = String::new();
    let mut array_items: Vec<String> = Vec::new();

    for line in content.lines() {
        let trimmed = line.trim();
        if trimmed.is_empty() || trimmed.starts_with('#') {
            continue;
        }

        // Multi-line array continuation
        if in_array {
            if let Some(end_idx) = trimmed.find(']') {
                let part = &trimmed[..end_idx];
                extract_quoted_strings(part, &mut array_items);
                in_array = false;
                apply_array_field(&mut config, current_section, &array_key, &array_items, &mut current_check);
                array_items.clear();
            } else {
                extract_quoted_strings(trimmed, &mut array_items);
            }
            continue;
        }

        // Section header
        if trimmed.starts_with("[[") && trimmed.ends_with("]]") {
            if let Some(check) = current_check.take() {
                config.pre_tool_use.custom_checks.push(check);
            }
            let section_name = trimmed.trim_matches(['[', ']']).trim();
            current_section = if section_name == "pre_tool_use.custom_checks" {
                current_check = Some(CustomCheck {
                    name: String::new(),
                    matcher: "*".to_owned(),
                    command: Vec::new(),
                    fail_if_output_contains: None,
                    decision: HookDecision::Deny,
                    reason: "Declarative check condition failed".to_owned(),
                });
                "custom_check"
            } else {
                "unknown"
            };
            continue;
        }

        if trimmed.starts_with('[') && trimmed.ends_with(']') {
            if let Some(check) = current_check.take() {
                config.pre_tool_use.custom_checks.push(check);
            }
            let section_name = trimmed.trim_matches(['[', ']']).trim();
            current_section = if section_name == "pre_tool_use" {
                "pre_tool_use"
            } else {
                "unknown"
            };
            continue;
        }

        // Key = Value parsing
        if let Some((key_part, val_part)) = trimmed.split_once('=') {
            let key = key_part.trim();
            let val = val_part.trim();

            if let Some(rest) = val.strip_prefix('[') {
                if let Some(end_idx) = rest.find(']') {
                    let inside = rest.get(..end_idx).unwrap_or("");
                    let mut items = Vec::new();
                    extract_quoted_strings(inside, &mut items);
                    apply_array_field(&mut config, current_section, key, &items, &mut current_check);
                } else {
                    in_array = true;
                    key.clone_into(&mut array_key);
                    extract_quoted_strings(rest, &mut array_items);
                }
            } else {
                let clean_val = val.trim_matches(['"', '\'']);
                apply_string_field(&mut config, current_section, key, clean_val, &mut current_check);
            }
        }
    }

    if let Some(check) = current_check {
        config.pre_tool_use.custom_checks.push(check);
    }

    config
}

fn extract_quoted_strings(slice: &str, items: &mut Vec<String>) {
    let mut in_quote = false;
    let mut current = String::new();
    let mut quote_char = '"';

    for ch in slice.chars() {
        if in_quote {
            if ch == quote_char {
                in_quote = false;
                items.push(current.clone());
                current.clear();
            } else {
                current.push(ch);
            }
        } else if ch == '"' || ch == '\'' {
            in_quote = true;
            quote_char = ch;
        }
    }
}

fn apply_array_field(
    config: &mut HooksConfig,
    section: &str,
    key: &str,
    items: &[String],
    current_check: &mut Option<CustomCheck>,
) {
    if section == "pre_tool_use" && key == "pipeline" {
        config.pre_tool_use.pipeline = items.to_vec();
    } else if section == "custom_check"
        && key == "command"
        && let Some(check) = current_check
    {
        check.command = items.to_vec();
    }
}

fn apply_string_field(
    config: &mut HooksConfig,
    section: &str,
    key: &str,
    val: &str,
    current_check: &mut Option<CustomCheck>,
) {
    if section == "pre_tool_use" && key == "vcs_conflicts_matcher" {
        val.clone_into(&mut config.pre_tool_use.vcs_conflicts_matcher);
    } else if section == "circuit_breaker" && key == "max_strikes" {
        if let Ok(m) = val.parse::<usize>() {
            config.circuit_breaker.max_strikes = m;
        }
    } else if section == "custom_check"
        && let Some(check) = current_check
    {
        match key {
            "name" => val.clone_into(&mut check.name),
            "matcher" => val.clone_into(&mut check.matcher),
            "fail_if_output_contains" => check.fail_if_output_contains = Some(val.to_owned()),
            "decision" => {
                check.decision = if val.eq_ignore_ascii_case("ask") {
                    HookDecision::Ask
                } else {
                    HookDecision::Deny
                };
            }
            "reason" => val.clone_into(&mut check.reason),
            _ => {}
        }
    }
}

#[cfg(test)]
#[allow(clippy::unwrap_used, clippy::indexing_slicing, reason = "Unit test assertions and indexing")]
mod tests {
    use super::*;

    #[test]
    fn test_matches_pattern_wildcard() {
        assert!(matches_pattern("anything", "*"));
        assert!(matches_pattern("anything", ""));
        assert!(matches_pattern("run_cargo_check", "run_cargo_.*|run_quick_check"));
        assert!(matches_pattern("run_quick_check", "run_cargo_.*|run_quick_check"));
        assert!(!matches_pattern("modify_code", "run_cargo_.*|run_quick_check"));
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
vcs_conflicts_matcher = "run_cargo_.*|custom_tool"

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
        assert_eq!(parsed.pre_tool_use.vcs_conflicts_matcher, "run_cargo_.*|custom_tool");
        assert_eq!(parsed.pre_tool_use.custom_checks.len(), 1);
        assert_eq!(parsed.pre_tool_use.custom_checks[0].name, "check_disk");
        assert_eq!(parsed.pre_tool_use.custom_checks[0].matcher, "modify_code|write_file");
        assert_eq!(parsed.pre_tool_use.custom_checks[0].command, vec!["df", "-h", "."]);
        assert_eq!(parsed.pre_tool_use.custom_checks[0].fail_if_output_contains.as_deref(), Some("100%"));
        assert_eq!(parsed.pre_tool_use.custom_checks[0].decision, HookDecision::Deny);
    }
}
