//! # project
//!
//! Declarative `project.toml` configuration model, parser, and ecosystem auto-detection.
//! Decouples language-specific build tools, compilers, test runners, and build caches
//! from the `agy-suite` core engine.

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};
use serde::{Deserialize, Serialize};

/// Default execution timeout for quality and compilation gates (in seconds).
pub const DEFAULT_GATE_TIMEOUT_SECS: u64 = 30;

/// Default timeout for test compilation gates (in seconds).
pub const DEFAULT_TEST_GATE_TIMEOUT_SECS: u64 = 45;

/// Metadata describing the project identity and primary ecosystem.
#[derive(Debug, Clone, PartialEq, Eq, Default, Serialize, Deserialize)]
pub struct ProjectMeta {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub name: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub language: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub version: Option<String>,
}

/// A declarative gate check definition (e.g. compilation, unit tests, linter).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct GateDefinition {
    pub command: Vec<String>,
    #[serde(default = "default_gate_timeout")]
    pub timeout_seconds: u64,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub failure_message: Option<String>,
}

const fn default_gate_timeout() -> u64 {
    DEFAULT_GATE_TIMEOUT_SECS
}

/// Declarative sandboxing configuration for Bubblewrap jail mounts.
#[derive(Debug, Clone, PartialEq, Eq, Default, Serialize, Deserialize)]
pub struct ProjectSandboxConfig {
    #[serde(default)]
    pub bind_mounts: Vec<String>,
}

/// Security category that can be selectively blocked or permitted.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
pub enum SecurityCategory {
    GenericCredentials,
    VcsInternals,
    SshKeys,
    ShellProfiles,
    SensitiveExtensions,
}

/// Declarative security & sensitive file protection configuration for the project.
#[derive(Debug, Clone, PartialEq, Eq, Default, Serialize, Deserialize)]
pub struct ProjectSecurityConfig {
    /// Categories of sensitive files explicitly disabled/permitted.
    #[serde(default)]
    pub disabled_categories: Vec<String>,
    /// Additional banned patterns or filenames specified per project.
    #[serde(default)]
    pub extra_banned_patterns: Vec<String>,
    /// Patterns explicitly allowed to bypass generic credential or extension blocks.
    #[serde(default)]
    pub allowed_patterns: Vec<String>,
}

impl ProjectSecurityConfig {
    /// Returns whether the specified security category is currently blocked.
    #[must_use]
    pub fn is_category_blocked(&self, category: SecurityCategory) -> bool {
        let name = match category {
            SecurityCategory::GenericCredentials => "generic_credentials",
            SecurityCategory::VcsInternals => "vcs_internals",
            SecurityCategory::SshKeys => "ssh_keys",
            SecurityCategory::ShellProfiles => "shell_profiles",
            SecurityCategory::SensitiveExtensions => "sensitive_extensions",
        };
        !self.disabled_categories.iter().any(|c| c.eq_ignore_ascii_case(name))
    }

    /// Whether generic words like "password", "credential", "secret" are blocked. Default: true.
    #[must_use]
    pub fn block_generic_credentials(&self) -> bool {
        self.is_category_blocked(SecurityCategory::GenericCredentials)
    }

    /// Whether VCS internals like ".git", ".jj" are blocked. Default: true.
    #[must_use]
    pub fn block_vcs_internals(&self) -> bool {
        self.is_category_blocked(SecurityCategory::VcsInternals)
    }

    /// Whether SSH keys like `id_rsa`, `id_ed25519` are blocked. Default: true.
    #[must_use]
    pub fn block_ssh_keys(&self) -> bool {
        self.is_category_blocked(SecurityCategory::SshKeys)
    }

    /// Whether shell profiles like ".bashrc", ".zshrc", ".profile" are blocked. Default: true.
    #[must_use]
    pub fn block_shell_profiles(&self) -> bool {
        self.is_category_blocked(SecurityCategory::ShellProfiles)
    }

    /// Whether file extensions like ".key", ".pem" are blocked. Default: true.
    #[must_use]
    pub fn block_sensitive_extensions(&self) -> bool {
        self.is_category_blocked(SecurityCategory::SensitiveExtensions)
    }

    /// Updates the blocked status for a specific category.
    pub fn set_category_blocked(&mut self, category: SecurityCategory, blocked: bool) {
        let name = match category {
            SecurityCategory::GenericCredentials => "generic_credentials",
            SecurityCategory::VcsInternals => "vcs_internals",
            SecurityCategory::SshKeys => "ssh_keys",
            SecurityCategory::ShellProfiles => "shell_profiles",
            SecurityCategory::SensitiveExtensions => "sensitive_extensions",
        };
        if blocked {
            self.disabled_categories.retain(|c| !c.eq_ignore_ascii_case(name));
        } else if !self.disabled_categories.iter().any(|c| c.eq_ignore_ascii_case(name)) {
            self.disabled_categories.push(name.to_owned());
        }
    }
}

/// Root project descriptor parsed from `project.toml` or dynamically synthesized.
#[derive(Debug, Clone, PartialEq, Eq, Default, Serialize, Deserialize)]
pub struct ProjectConfig {
    #[serde(default)]
    pub project: ProjectMeta,
    #[serde(default)]
    pub gates: BTreeMap<String, GateDefinition>,
    #[serde(default)]
    pub sandbox: ProjectSandboxConfig,
    #[serde(default)]
    pub environment: BTreeMap<String, String>,
    #[serde(default)]
    pub security: ProjectSecurityConfig,
    #[serde(default)]
    pub extra_banned_files: Vec<String>,
}

impl ProjectConfig {
    /// Loads `<root>/.agy/project.toml` or `<root>/project.toml`.
    /// If no explicit descriptor is found, dynamically infers the matching ecosystem profile
    /// based on root indicator files (`Cargo.toml`, `build.zig`, `go.mod`, etc.).
    #[must_use]
    pub fn detect_or_load(workspace_root: &Path) -> Self {
        let candidates = [
            workspace_root.join(".agy/project.toml"),
            workspace_root.join("project.toml"),
        ];

        for path in &candidates {
            if let Ok(content) = std::fs::read_to_string(path)
                && let Ok(cfg) = parse_project_toml(&content)
            {
                return cfg;
            }
        }

        Self::infer_from_workspace(workspace_root)
    }

    /// Infers default compiler gates, cache bind-mounts, and environment variables
    /// based on recognized repository manifest indicators.
    #[must_use]
    pub fn infer_from_workspace(workspace_root: &Path) -> Self {
        if workspace_root.join("Cargo.toml").is_file() {
            Self::infer_rust()
        } else if workspace_root.join("build.zig").is_file() {
            Self::infer_zig()
        } else if workspace_root.join("go.mod").is_file() {
            Self::infer_go()
        } else if workspace_root.join("package.json").is_file() {
            Self::infer_typescript()
        } else if workspace_root.join("pyproject.toml").is_file() {
            Self::infer_python()
        } else if workspace_root.join("meson.build").is_file() {
            Self::infer_meson()
        } else if workspace_root.join("CMakeLists.txt").is_file() {
            Self::infer_cmake()
        } else {
            Self::default()
        }
    }

    fn infer_rust() -> Self {
        let mut gates = BTreeMap::new();
        gates.insert(
            "compilation".to_owned(),
            GateDefinition {
                command: vec!["cargo".into(), "check".into(), "--all-targets".into()],
                timeout_seconds: DEFAULT_GATE_TIMEOUT_SECS,
                failure_message: Some(
                    "Project compilation failed (`cargo check --all-targets`).".into(),
                ),
            },
        );
        gates.insert(
            "tests".to_owned(),
            GateDefinition {
                command: vec!["cargo".into(), "test".into(), "--no-run".into()],
                timeout_seconds: DEFAULT_TEST_GATE_TIMEOUT_SECS,
                failure_message: Some(
                    "Test suite compilation failed (`cargo test --no-run`).".into(),
                ),
            },
        );
        gates.insert(
            "linter".to_owned(),
            GateDefinition {
                command: vec![
                    "cargo".into(),
                    "clippy".into(),
                    "--all-targets".into(),
                    "--".into(),
                    "-D".into(),
                    "warnings".into(),
                ],
                timeout_seconds: DEFAULT_GATE_TIMEOUT_SECS,
                failure_message: Some("Linter check failed (`cargo clippy`).".into()),
            },
        );

        let mut environment = BTreeMap::new();
        environment.insert("CARGO_TARGET_DIR".into(), "target".into());

        Self {
            project: ProjectMeta {
                name: None,
                language: Some("rust".into()),
                version: None,
            },
            gates,
            sandbox: ProjectSandboxConfig {
                bind_mounts: vec!["target".into()],
            },
            environment,
            security: ProjectSecurityConfig::default(),
            extra_banned_files: Vec::new(),
        }
    }

    fn infer_zig() -> Self {
        let mut gates = BTreeMap::new();
        gates.insert(
            "compilation".to_owned(),
            GateDefinition {
                command: vec!["zig".into(), "build".into()],
                timeout_seconds: DEFAULT_GATE_TIMEOUT_SECS,
                failure_message: Some("Zig project build failed (`zig build`).".into()),
            },
        );
        gates.insert(
            "tests".to_owned(),
            GateDefinition {
                command: vec!["zig".into(), "build".into(), "test".into(), "--no-exec".into()],
                timeout_seconds: DEFAULT_TEST_GATE_TIMEOUT_SECS,
                failure_message: Some("Zig test build failed (`zig build test --no-exec`).".into()),
            },
        );

        Self {
            project: ProjectMeta {
                name: None,
                language: Some("zig".into()),
                version: None,
            },
            gates,
            sandbox: ProjectSandboxConfig {
                bind_mounts: vec![".zig-cache".into(), "zig-out".into()],
            },
            environment: BTreeMap::new(),
            security: ProjectSecurityConfig::default(),
            extra_banned_files: Vec::new(),
        }
    }

    fn infer_go() -> Self {
        let mut gates = BTreeMap::new();
        gates.insert(
            "compilation".to_owned(),
            GateDefinition {
                command: vec!["go".into(), "build".into(), "./...".into()],
                timeout_seconds: DEFAULT_GATE_TIMEOUT_SECS,
                failure_message: Some("Go package build failed (`go build ./...`).".into()),
            },
        );
        gates.insert(
            "tests".to_owned(),
            GateDefinition {
                command: vec!["go".into(), "test".into(), "-run=^$".into(), "./...".into()],
                timeout_seconds: DEFAULT_TEST_GATE_TIMEOUT_SECS,
                failure_message: Some("Go test compilation failed (`go test -run=^$ ./...`).".into()),
            },
        );

        Self {
            project: ProjectMeta {
                name: None,
                language: Some("go".into()),
                version: None,
            },
            gates,
            sandbox: ProjectSandboxConfig {
                bind_mounts: vec![".cache/go-build".into()],
            },
            environment: BTreeMap::new(),
            security: ProjectSecurityConfig::default(),
            extra_banned_files: Vec::new(),
        }
    }

    fn infer_typescript() -> Self {
        let mut gates = BTreeMap::new();
        gates.insert(
            "compilation".to_owned(),
            GateDefinition {
                command: vec!["pnpm".into(), "typecheck".into()],
                timeout_seconds: DEFAULT_GATE_TIMEOUT_SECS,
                failure_message: Some("TypeScript typecheck failed (`pnpm typecheck`).".into()),
            },
        );

        Self {
            project: ProjectMeta {
                name: None,
                language: Some("typescript".into()),
                version: None,
            },
            gates,
            sandbox: ProjectSandboxConfig {
                bind_mounts: vec![".turbo".into(), "node_modules/.cache".into()],
            },
            environment: BTreeMap::new(),
            security: ProjectSecurityConfig::default(),
            extra_banned_files: Vec::new(),
        }
    }

    fn infer_python() -> Self {
        let mut gates = BTreeMap::new();
        gates.insert(
            "compilation".to_owned(),
            GateDefinition {
                command: vec!["mypy".into(), ".".into()],
                timeout_seconds: DEFAULT_GATE_TIMEOUT_SECS,
                failure_message: Some("Python typecheck failed (`mypy .`).".into()),
            },
        );

        Self {
            project: ProjectMeta {
                name: None,
                language: Some("python".into()),
                version: None,
            },
            gates,
            sandbox: ProjectSandboxConfig {
                bind_mounts: vec![".pytest_cache".into(), ".mypy_cache".into()],
            },
            environment: BTreeMap::new(),
            security: ProjectSecurityConfig::default(),
            extra_banned_files: Vec::new(),
        }
    }

    fn infer_meson() -> Self {
        let mut gates = BTreeMap::new();
        gates.insert(
            "compilation".to_owned(),
            GateDefinition {
                command: vec!["meson".into(), "compile".into(), "-C".into(), "build".into()],
                timeout_seconds: DEFAULT_GATE_TIMEOUT_SECS,
                failure_message: Some("Meson build compilation failed (`meson compile -C build`).".into()),
            },
        );

        Self {
            project: ProjectMeta {
                name: None,
                language: Some("c".into()),
                version: None,
            },
            gates,
            sandbox: ProjectSandboxConfig {
                bind_mounts: vec!["build".into()],
            },
            environment: BTreeMap::new(),
            security: ProjectSecurityConfig::default(),
            extra_banned_files: Vec::new(),
        }
    }

    fn infer_cmake() -> Self {
        let mut gates = BTreeMap::new();
        gates.insert(
            "compilation".to_owned(),
            GateDefinition {
                command: vec!["cmake".into(), "--build".into(), "build".into()],
                timeout_seconds: DEFAULT_GATE_TIMEOUT_SECS,
                failure_message: Some("CMake build compilation failed (`cmake --build build`).".into()),
            },
        );

        Self {
            project: ProjectMeta {
                name: None,
                language: Some("cpp".into()),
                version: None,
            },
            gates,
            sandbox: ProjectSandboxConfig {
                bind_mounts: vec!["build".into()],
            },
            environment: BTreeMap::new(),
            security: ProjectSecurityConfig::default(),
            extra_banned_files: Vec::new(),
        }
    }

    /// Resolves extra cache directories that should be bind-mounted into Bubblewrap.
    #[must_use]
    pub fn resolved_cache_binds(&self, workspace_root: &Path) -> Vec<PathBuf> {
        let mut paths = Vec::new();
        for rel in &self.sandbox.bind_mounts {
            let p = workspace_root.join(rel);
            if p.is_dir() {
                paths.push(p);
            }
        }
        paths
    }
}

/// Parses a TOML string into a structured `ProjectConfig`.
/// Uses a lightweight, robust line parser that does not require additional crate dependencies.
///
/// # Errors
/// Returns an error message if the content fails syntax validation.
#[allow(clippy::string_slice, reason = "Line parser operates on verified character delimiters")]
pub fn parse_project_toml(content: &str) -> Result<ProjectConfig, String> {
    let mut config = ProjectConfig::default();
    let mut current_section = String::new();

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
                if let Some(part) = trimmed.get(..end_idx) {
                    extract_quoted_items(part, &mut array_items);
                }
                in_array = false;
                apply_array_field(&mut config, &current_section, &array_key, &array_items);
                array_items.clear();
            } else {
                extract_quoted_items(trimmed, &mut array_items);
            }
            continue;
        }

        // Section header
        if let Some(rest) = trimmed.strip_prefix('[')
            && let Some(section) = rest.strip_suffix(']')
        {
            section.trim().clone_into(&mut current_section);
            continue;
        }

        // Key-value pair
        if let Some((key_raw, val_raw)) = trimmed.split_once('=') {
            let key = key_raw.trim();
            let val = val_raw.trim();

            if let Some(inner) = val.strip_prefix('[') {
                if let Some(end_idx) = inner.find(']') {
                    let mut items = Vec::new();
                    if let Some(part) = inner.get(..end_idx) {
                        extract_quoted_items(part, &mut items);
                    }
                    apply_array_field(&mut config, &current_section, key, &items);
                } else {
                    in_array = true;
                    key.clone_into(&mut array_key);
                    array_items.clear();
                    extract_quoted_items(inner, &mut array_items);
                }
            } else {
                apply_scalar_field(&mut config, &current_section, key, val);
            }
        }
    }

    Ok(config)
}

fn extract_quoted_items(text: &str, items: &mut Vec<String>) {
    let mut chars = text.chars();
    while let Some(c) = chars.next() {
        if c == '"' {
            let mut s = String::new();
            while let Some(inner) = chars.next() {
                if inner == '"' {
                    break;
                }
                if inner == '\\' {
                    if let Some(escaped) = chars.next() {
                        s.push(escaped);
                    }
                } else {
                    s.push(inner);
                }
            }
            items.push(s);
        }
    }
}

fn apply_scalar_field(config: &mut ProjectConfig, section: &str, key: &str, val: &str) {
    let clean_str = val.trim_matches('"');

    if section == "project" {
        match key {
            "name" => config.project.name = Some(clean_str.to_owned()),
            "language" => config.project.language = Some(clean_str.to_owned()),
            "version" => config.project.version = Some(clean_str.to_owned()),
            _ => {}
        }
    } else if let Some(gate_name) = section.strip_prefix("gates.") {
        let gate = config.gates.entry(gate_name.to_owned()).or_insert_with(|| GateDefinition {
            command: Vec::new(),
            timeout_seconds: DEFAULT_GATE_TIMEOUT_SECS,
            failure_message: None,
        });

        match key {
            "timeout_seconds" => {
                if let Ok(t) = clean_str.parse::<u64>() {
                    gate.timeout_seconds = t;
                }
            }
            "failure_message" => {
                gate.failure_message = Some(clean_str.to_owned());
            }
            _ => {}
        }
    } else if section == "environment" {
        config.environment.insert(key.to_owned(), clean_str.to_owned());
    } else if section == "security" {
        let is_true = clean_str == "true";
        match key {
            "block_generic_credentials" => config.security.set_category_blocked(SecurityCategory::GenericCredentials, is_true),
            "block_vcs_internals" => config.security.set_category_blocked(SecurityCategory::VcsInternals, is_true),
            "block_ssh_keys" => config.security.set_category_blocked(SecurityCategory::SshKeys, is_true),
            "block_shell_profiles" => config.security.set_category_blocked(SecurityCategory::ShellProfiles, is_true),
            "block_sensitive_extensions" => config.security.set_category_blocked(SecurityCategory::SensitiveExtensions, is_true),
            _ => {}
        }
    }
}

fn apply_array_field(config: &mut ProjectConfig, section: &str, key: &str, items: &[String]) {
    if key == "extra_banned_files" {
        config.extra_banned_files = items.to_vec();
        config.security.extra_banned_patterns.extend(items.to_vec());
    } else if let Some(gate_name) = section.strip_prefix("gates.") {
        let gate = config.gates.entry(gate_name.to_owned()).or_insert_with(|| GateDefinition {
            command: Vec::new(),
            timeout_seconds: DEFAULT_GATE_TIMEOUT_SECS,
            failure_message: None,
        });

        if key == "command" {
            gate.command = items.to_vec();
        }
    } else if section == "sandbox" && key == "bind_mounts" {
        config.sandbox.bind_mounts = items.to_vec();
    } else if section == "security" {
        if key == "extra_banned_patterns" || key == "extra_banned_files" {
            config.security.extra_banned_patterns = items.to_vec();
        } else if key == "allowed_patterns" {
            config.security.allowed_patterns = items.to_vec();
        } else if key == "disabled_categories" {
            config.security.disabled_categories = items.to_vec();
        }
    }
}

#[cfg(test)]
#[allow(
    clippy::expect_used,
    clippy::unwrap_used,
    clippy::indexing_slicing,
    clippy::let_underscore_must_use,
    reason = "Unit test assertions"
)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_project_toml_full() {
        let sample = r#"
[project]
name = "riscv-core"
language = "rust"
version = "0.1.0"

[gates.compilation]
command = ["cargo", "check", "--all-targets"]
timeout_seconds = 35
failure_message = "RISC-V emulator compilation failed"

[gates.tests]
command = ["cargo", "test", "--no-run"]
timeout_seconds = 50

[sandbox]
bind_mounts = ["target", "build/out"]

[environment]
CARGO_TARGET_DIR = "target"
RISCV_TESTS_DIR = "fixtures/tests"

extra_banned_files = ["firmware_key.pem", ".secret_payload"]
"#;

        let parsed = parse_project_toml(sample).unwrap();
        assert_eq!(parsed.project.name.as_deref(), Some("riscv-core"));
        assert_eq!(parsed.project.language.as_deref(), Some("rust"));

        let comp = &parsed.gates["compilation"];
        assert_eq!(comp.command, vec!["cargo", "check", "--all-targets"]);
        assert_eq!(comp.timeout_seconds, 35);
        assert_eq!(
            comp.failure_message.as_deref(),
            Some("RISC-V emulator compilation failed")
        );

        let test_gate = &parsed.gates["tests"];
        assert_eq!(test_gate.command, vec!["cargo", "test", "--no-run"]);
        assert_eq!(test_gate.timeout_seconds, 50);

        assert_eq!(parsed.sandbox.bind_mounts, vec!["target", "build/out"]);
        assert_eq!(
            parsed.environment.get("CARGO_TARGET_DIR").map(String::as_str),
            Some("target")
        );
        assert_eq!(
            parsed.extra_banned_files,
            vec!["firmware_key.pem", ".secret_payload"]
        );
    }

    #[test]
    fn test_infer_from_workspace_rust() {
        let temp = std::env::temp_dir().join(format!("agy_proj_test_{}", std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).unwrap().as_nanos()));
        std::fs::create_dir_all(&temp).unwrap();
        std::fs::write(temp.join("Cargo.toml"), "[package]\nname = \"dummy\"\n").unwrap();

        let cfg = ProjectConfig::infer_from_workspace(&temp);
        assert_eq!(cfg.project.language.as_deref(), Some("rust"));
        assert!(cfg.gates.contains_key("compilation"));
        assert!(cfg.gates.contains_key("tests"));
        assert!(cfg.gates.contains_key("linter"));
        assert_eq!(cfg.sandbox.bind_mounts, vec!["target"]);

        drop(std::fs::remove_dir_all(&temp));
    }

    #[test]
    fn test_infer_from_workspace_zig() {
        let temp = std::env::temp_dir().join(format!("agy_zig_test_{}", std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).unwrap().as_nanos()));
        std::fs::create_dir_all(&temp).unwrap();
        std::fs::write(temp.join("build.zig"), "const std = @import(\"std\");\n").unwrap();

        let cfg = ProjectConfig::infer_from_workspace(&temp);
        assert_eq!(cfg.project.language.as_deref(), Some("zig"));
        assert_eq!(cfg.gates["compilation"].command, vec!["zig", "build"]);
        assert_eq!(cfg.sandbox.bind_mounts, vec![".zig-cache", "zig-out"]);

        drop(std::fs::remove_dir_all(&temp));
    }

    #[test]
    fn test_parse_security_config() {
        let sample = r#"
[project]
name = "auth-service"

[security]
block_generic_credentials = false
block_sensitive_extensions = false
block_ssh_keys = true
allowed_patterns = ["src/auth/password/**", "tests/fixtures/*.pem"]
extra_banned_patterns = ["confidential_leak.bin"]
"#;

        let parsed = parse_project_toml(sample).unwrap();
        assert!(!parsed.security.block_generic_credentials());
        assert!(!parsed.security.block_sensitive_extensions());
        assert!(parsed.security.block_ssh_keys());
        assert_eq!(
            parsed.security.allowed_patterns,
            vec!["src/auth/password/**", "tests/fixtures/*.pem"]
        );
        assert_eq!(
            parsed.security.extra_banned_patterns,
            vec!["confidential_leak.bin"]
        );
    }
}
