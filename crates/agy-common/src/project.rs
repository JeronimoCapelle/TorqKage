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
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub error_regex: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub explain_command: Option<Vec<String>>,
}

const fn default_gate_timeout() -> u64 {
    DEFAULT_GATE_TIMEOUT_SECS
}

/// Declarative workspace layout and ignored cache directories.
#[derive(Debug, Clone, PartialEq, Eq, Default, Serialize, Deserialize)]
pub struct ProjectWorkspaceConfig {
    /// Monorepo container directory names (e.g. `["crates", "packages", "apps", "libs"]`).
    #[serde(default)]
    pub containers: Vec<String>,
    /// Additional build cache or ignored paths for VCS and diff checks (e.g. `["target", ".zig-cache", "build"]`).
    #[serde(default)]
    pub ignored_paths: Vec<String>,
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

/// Declarative documentation and specification paths for the project.
#[derive(Debug, Clone, PartialEq, Eq, Default, Serialize, Deserialize)]
pub struct ProjectDocsConfig {
    /// List of paths (directories or files) containing project documentation and specifications.
    #[serde(default)]
    pub paths: Vec<String>,
    /// Optional description or instructions for agents on how to use the documentation.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub description: Option<String>,
    /// Optional tool name recommended to agents for searching documentation (e.g. "`query_project_spec`").
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub tool_name: Option<String>,
}

impl ProjectDocsConfig {
    /// Returns the configured documentation paths.
    #[must_use]
    pub fn resolved_paths(&self) -> &[String] {
        &self.paths
    }

    /// Formats a human-readable list of documentation paths for agent awareness.
    #[must_use]
    pub fn display_summary(&self) -> String {
        if self.paths.is_empty() {
            "None configured".to_owned()
        } else {
            self.paths.join(", ")
        }
    }
}

/// Declarative project-level path policies and language syntax configurations.
#[derive(Debug, Clone, PartialEq, Eq, Default, Serialize, Deserialize)]
pub struct ProjectPolicyConfig {
    #[serde(default)]
    pub syntax: Option<crate::content_filter::SyntaxConfig>,
    #[serde(default)]
    pub path_policies: Vec<crate::content_filter::PathPolicy>,
}

/// Root project descriptor parsed from `project.toml` or dynamically synthesized.
#[derive(Debug, Clone, PartialEq, Eq, Default, Serialize, Deserialize)]
pub struct ProjectConfig {
    #[serde(default)]
    pub project: ProjectMeta,
    #[serde(default)]
    pub workspace: ProjectWorkspaceConfig,
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
    #[serde(default)]
    pub docs: ProjectDocsConfig,
    #[serde(default)]
    pub policies: ProjectPolicyConfig,
    #[serde(default)]
    pub path_policies: Vec<crate::content_filter::PathPolicy>,
}

impl ProjectConfig {
    /// Returns all declared path policies, combining `path_policies` and `policies.path_policies`.
    #[must_use]
    pub fn all_path_policies(&self) -> Vec<crate::content_filter::PathPolicy> {
        let mut all = self.path_policies.clone();
        for p in &self.policies.path_policies {
            if !all.contains(p) {
                all.push(p.clone());
            }
        }
        all
    }

    /// Returns all build cache and ignored directory paths across `sandbox.bind_mounts` and `workspace.ignored_paths`.
    #[must_use]
    pub fn all_ignored_or_cache_paths(&self) -> Vec<String> {
        let mut paths = self.sandbox.bind_mounts.clone();
        for p in &self.workspace.ignored_paths {
            if !paths.contains(p) {
                paths.push(p.clone());
            }
        }
        paths
    }

    /// Checks if a relative path component or prefix matches any declared ignored or build cache directory.
    #[must_use]
    pub fn is_ignored_or_cache_path(&self, rel_path: &str) -> bool {
        let clean = rel_path.trim().replace('\\', "/");
        let all = self.all_ignored_or_cache_paths();
        for p in all {
            let p_clean = p.trim_matches(['/', '\\']).replace('\\', "/");
            if clean == p_clean
                || clean.starts_with(&format!("{p_clean}/"))
                || clean.ends_with(&format!("/{p_clean}"))
            {
                return true;
            }
        }
        false
    }

    /// Returns the resolved monorepo containers, using explicit configuration from `project.toml`
    /// or detecting existing directory indicators on disk (`crates`, `packages`).
    #[must_use]
    pub fn resolved_containers(&self, workspace_root: &Path) -> Vec<String> {
        if !self.workspace.containers.is_empty() {
            return self.workspace.containers.clone();
        }
        let mut detected = Vec::new();
        for dir in ["crates", "packages"] {
            if workspace_root.join(dir).is_dir() {
                detected.push(dir.to_owned());
            }
        }
        detected
    }

    /// Auto-detects standard documentation directories and architecture files in the workspace.
    #[must_use]
    pub fn detect_default_docs_paths(workspace_root: &Path) -> Vec<String> {
        let mut paths = Vec::new();
        for candidate in ["docs", "doc", "specs", "spec", "documentation"] {
            if workspace_root.join(candidate).is_dir() {
                paths.push(candidate.to_owned());
            }
        }
        for file_candidate in ["ARCHITECTURE.md", "SPEC.md"] {
            if workspace_root.join(file_candidate).is_file() {
                paths.push(file_candidate.to_owned());
            }
        }
        paths
    }

    /// Loads `<root>/.agy/project.toml`, `<root>/project.toml`, or `<root>/plugins/agy-suite/project.toml`.
    /// If no explicit descriptor is found, falls back to a blank/restrictive default configuration
    /// with auto-detected documentation paths.
    #[must_use]
    pub fn detect_or_load(workspace_root: &Path) -> Self {
        let candidates = [
            workspace_root.join(".agy/project.toml"),
            workspace_root.join("project.toml"),
            workspace_root.join("plugins/agy-suite/project.toml"),
        ];

        for path in &candidates {
            if let Ok(content) = std::fs::read_to_string(path)
                && let Ok(mut cfg) = parse_project_toml(&content)
            {
                if cfg.docs.paths.is_empty() {
                    cfg.docs.paths = Self::detect_default_docs_paths(workspace_root);
                }
                return cfg;
            }
        }

        let mut cfg = Self::default();
        cfg.docs.paths = Self::detect_default_docs_paths(workspace_root);
        cfg
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
                error_regex: Some("error[E".into()),
                explain_command: Some(vec!["rustc".into(), "--explain".into()]),
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
                error_regex: Some("error[E".into()),
                explain_command: Some(vec!["rustc".into(), "--explain".into()]),
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
                error_regex: Some("error[E".into()),
                explain_command: None,
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
            workspace: ProjectWorkspaceConfig {
                containers: vec!["crates".into(), "packages".into()],
                ignored_paths: vec!["target".into()],
            },
            gates,
            sandbox: ProjectSandboxConfig {
                bind_mounts: vec!["target".into()],
            },
            environment,
            security: ProjectSecurityConfig::default(),
            extra_banned_files: Vec::new(),
            docs: ProjectDocsConfig::default(),
            ..Default::default()
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
                error_regex: None,
                explain_command: None,
            },
        );
        gates.insert(
            "tests".to_owned(),
            GateDefinition {
                command: vec!["zig".into(), "build".into(), "test".into(), "--no-exec".into()],
                timeout_seconds: DEFAULT_TEST_GATE_TIMEOUT_SECS,
                failure_message: Some("Zig test build failed (`zig build test --no-exec`).".into()),
                error_regex: None,
                explain_command: None,
            },
        );

        Self {
            project: ProjectMeta {
                name: None,
                language: Some("zig".into()),
                version: None,
            },
            workspace: ProjectWorkspaceConfig {
                containers: Vec::new(),
                ignored_paths: vec![".zig-cache".into(), "zig-out".into()],
            },
            gates,
            sandbox: ProjectSandboxConfig {
                bind_mounts: vec![".zig-cache".into(), "zig-out".into()],
            },
            environment: BTreeMap::new(),
            security: ProjectSecurityConfig::default(),
            extra_banned_files: Vec::new(),
            docs: ProjectDocsConfig::default(),
            ..Default::default()
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
                error_regex: None,
                explain_command: None,
            },
        );
        gates.insert(
            "tests".to_owned(),
            GateDefinition {
                command: vec!["go".into(), "test".into(), "-run=^$".into(), "./...".into()],
                timeout_seconds: DEFAULT_TEST_GATE_TIMEOUT_SECS,
                failure_message: Some("Go test compilation failed (`go test -run=^$ ./...`).".into()),
                error_regex: None,
                explain_command: None,
            },
        );

        Self {
            project: ProjectMeta {
                name: None,
                language: Some("go".into()),
                version: None,
            },
            workspace: ProjectWorkspaceConfig {
                containers: Vec::new(),
                ignored_paths: vec![".cache/go-build".into()],
            },
            gates,
            sandbox: ProjectSandboxConfig {
                bind_mounts: vec![".cache/go-build".into()],
            },
            environment: BTreeMap::new(),
            security: ProjectSecurityConfig::default(),
            extra_banned_files: Vec::new(),
            docs: ProjectDocsConfig::default(),
            ..Default::default()
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
                error_regex: None,
                explain_command: None,
            },
        );

        Self {
            project: ProjectMeta {
                name: None,
                language: Some("typescript".into()),
                version: None,
            },
            workspace: ProjectWorkspaceConfig {
                containers: vec!["packages".into(), "apps".into()],
                ignored_paths: vec![".turbo".into(), "node_modules/.cache".into(), "dist".into()],
            },
            gates,
            sandbox: ProjectSandboxConfig {
                bind_mounts: vec![".turbo".into(), "node_modules/.cache".into()],
            },
            environment: BTreeMap::new(),
            security: ProjectSecurityConfig::default(),
            extra_banned_files: Vec::new(),
            docs: ProjectDocsConfig::default(),
            ..Default::default()
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
                error_regex: None,
                explain_command: None,
            },
        );

        Self {
            project: ProjectMeta {
                name: None,
                language: Some("python".into()),
                version: None,
            },
            workspace: ProjectWorkspaceConfig {
                containers: Vec::new(),
                ignored_paths: vec![".pytest_cache".into(), ".mypy_cache".into(), "__pycache__".into()],
            },
            gates,
            sandbox: ProjectSandboxConfig {
                bind_mounts: vec![".pytest_cache".into(), ".mypy_cache".into()],
            },
            environment: BTreeMap::new(),
            security: ProjectSecurityConfig::default(),
            extra_banned_files: Vec::new(),
            docs: ProjectDocsConfig::default(),
            ..Default::default()
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
                error_regex: None,
                explain_command: None,
            },
        );

        Self {
            project: ProjectMeta {
                name: None,
                language: Some("c".into()),
                version: None,
            },
            workspace: ProjectWorkspaceConfig {
                containers: Vec::new(),
                ignored_paths: vec!["build".into()],
            },
            gates,
            sandbox: ProjectSandboxConfig {
                bind_mounts: vec!["build".into()],
            },
            environment: BTreeMap::new(),
            security: ProjectSecurityConfig::default(),
            extra_banned_files: Vec::new(),
            docs: ProjectDocsConfig::default(),
            ..Default::default()
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
                error_regex: None,
                explain_command: None,
            },
        );

        Self {
            project: ProjectMeta {
                name: None,
                language: Some("cpp".into()),
                version: None,
            },
            workspace: ProjectWorkspaceConfig {
                containers: Vec::new(),
                ignored_paths: vec!["build".into()],
            },
            gates,
            sandbox: ProjectSandboxConfig {
                bind_mounts: vec!["build".into()],
            },
            environment: BTreeMap::new(),
            security: ProjectSecurityConfig::default(),
            extra_banned_files: Vec::new(),
            docs: ProjectDocsConfig::default(),
            ..Default::default()
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

        // Table array header: e.g. [[path_policies]]
        if let Some(rest) = trimmed.strip_prefix("[[")
            && let Some(section) = rest.strip_suffix("]]")
        {
            let sec_name = section.trim();
            if sec_name == "path_policies" || sec_name == "policies.path_policies" {
                config.path_policies.push(crate::content_filter::PathPolicy::default());
                "path_policies".clone_into(&mut current_section);
                continue;
            }
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

fn unquote_toml_scalar(val: &str) -> &str {
    let trimmed = val.trim();
    if (trimmed.starts_with('"') && trimmed.ends_with('"') && trimmed.len() >= 2)
        || (trimmed.starts_with('\'') && trimmed.ends_with('\'') && trimmed.len() >= 2)
    {
        let end = trimmed.len().saturating_sub(1);
        trimmed.get(1..end).unwrap_or(trimmed)
    } else {
        trimmed
    }
}

fn apply_scalar_field(config: &mut ProjectConfig, section: &str, key: &str, val: &str) {
    let clean_str = unquote_toml_scalar(val);

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
            error_regex: None,
            explain_command: None,
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
            "error_regex" => {
                gate.error_regex = Some(clean_str.to_owned());
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
    } else if section == "docs" {
        match key {
            "description" => config.docs.description = Some(clean_str.to_owned()),
            "path" | "paths" => config.docs.paths = vec![clean_str.to_owned()],
            "tool_name" => config.docs.tool_name = Some(clean_str.to_owned()),
            _ => {}
        }
    } else if (section == "project" || section.is_empty()) && (key == "docs" || key == "docs_paths") {
        config.docs.paths = vec![clean_str.to_owned()];
    } else if section == "path_policies" {
        if let Some(last) = config.path_policies.last_mut() {
            match key {
                "agent" => last.agent = Some(clean_str.to_owned()),
                "path" => clean_str.clone_into(&mut last.path),
                "filter" => match clean_str {
                    "comments_only" | "comments" => {
                        last.filter = crate::content_filter::ContentFilterKind::CommentsOnly;
                    }
                    "tests_only" | "tests" => {
                        last.filter = crate::content_filter::ContentFilterKind::TestsOnly;
                    }
                    "delimited" => {
                        last.filter = crate::content_filter::ContentFilterKind::Delimited;
                    }
                    _ => {}
                },
                "start_marker" => last.start_marker = Some(clean_str.to_owned()),
                "end_marker" => last.end_marker = Some(clean_str.to_owned()),
                "line_comment" => last.line_comments = vec![clean_str.to_owned()],
                "block_comment_start" => {
                    if let Some(last_bc) = last.block_comments.last_mut() {
                        clean_str.clone_into(&mut last_bc.start);
                    } else {
                        last.block_comments.push(crate::content_filter::BlockCommentDelimiters::new(clean_str, ""));
                    }
                }
                "block_comment_end" => {
                    if let Some(last_bc) = last.block_comments.last_mut() {
                        clean_str.clone_into(&mut last_bc.end);
                    } else {
                        last.block_comments.push(crate::content_filter::BlockCommentDelimiters::new("", clean_str));
                    }
                }
                _ => {}
            }
        }
    } else if section == "policies.syntax" || section == "syntax" {
        let syntax = config.policies.syntax.get_or_insert_with(crate::content_filter::SyntaxConfig::default);
        match key {
            "line_comment" => syntax.line_comments = vec![clean_str.to_owned()],
            "test_prefix" => syntax.test_prefixes = vec![clean_str.to_owned()],
            "test_attribute" => syntax.test_attributes = vec![clean_str.to_owned()],
            _ => {}
        }
    }
}

fn apply_array_field(config: &mut ProjectConfig, section: &str, key: &str, items: &[String]) {
    if key == "extra_banned_files" {
        config.extra_banned_files = items.to_vec();
        config.security.extra_banned_patterns.extend(items.to_vec());
    } else if section == "workspace" {
        if key == "containers" {
            config.workspace.containers = items.to_vec();
        } else if key == "ignored_paths" {
            config.workspace.ignored_paths = items.to_vec();
        }
    } else if (section == "docs" && (key == "paths" || key == "dirs"))
        || ((section == "project" || section.is_empty())
            && (key == "docs" || key == "docs_paths" || key == "paths"))
    {
        config.docs.paths = items.to_vec();
    } else if let Some(gate_name) = section.strip_prefix("gates.") {
        let gate = config.gates.entry(gate_name.to_owned()).or_insert_with(|| GateDefinition {
            command: Vec::new(),
            timeout_seconds: DEFAULT_GATE_TIMEOUT_SECS,
            failure_message: None,
            error_regex: None,
            explain_command: None,
        });

        if key == "command" {
            gate.command = items.to_vec();
        } else if key == "explain_command" {
            gate.explain_command = Some(items.to_vec());
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
    } else if section == "path_policies" {
        if let Some(last) = config.path_policies.last_mut() {
            match key {
                "line_comments" => last.line_comments = items.to_vec(),
                "test_attributes" => last.test_attributes = items.to_vec(),
                "test_prefixes" => last.test_prefixes = items.to_vec(),
                _ => {}
            }
        }
    } else if section == "policies.syntax" || section == "syntax" {
        let syntax = config.policies.syntax.get_or_insert_with(crate::content_filter::SyntaxConfig::default);
        match key {
            "line_comments" => syntax.line_comments = items.to_vec(),
            "test_attributes" => syntax.test_attributes = items.to_vec(),
            "test_prefixes" => syntax.test_prefixes = items.to_vec(),
            _ => {}
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
    fn test_parse_project_toml_path_policies_and_syntax() {
        let sample = r##"
[project]
name = "python-service"
language = "python"

[policies.syntax]
line_comments = ["#"]
test_prefixes = ["test_"]

[[path_policies]]
agent = "doc-author"
path = "src"
filter = "comments_only"
block_comment_start = '"""'
block_comment_end = '"""'

[[path_policies]]
agent = "test-author"
path = "src"
filter = "tests_only"
"##;

        let parsed = parse_project_toml(sample).unwrap();
        assert_eq!(parsed.project.name.as_deref(), Some("python-service"));
        assert_eq!(parsed.project.language.as_deref(), Some("python"));

        let syntax = parsed.policies.syntax.as_ref().unwrap();
        assert_eq!(syntax.line_comments, vec!["#"]);
        assert_eq!(syntax.test_prefixes, vec!["test_"]);

        let policies = parsed.all_path_policies();
        assert_eq!(policies.len(), 2);
        assert_eq!(policies[0].agent.as_deref(), Some("doc-author"));
        assert_eq!(policies[0].path, "src");
        assert_eq!(policies[0].filter, crate::content_filter::ContentFilterKind::CommentsOnly);
        assert_eq!(policies[0].block_comments.len(), 1);
        assert_eq!(policies[0].block_comments[0].start, "\"\"\"");
        assert_eq!(policies[0].block_comments[0].end, "\"\"\"");

        assert_eq!(policies[1].agent.as_deref(), Some("test-author"));
        assert_eq!(policies[1].path, "src");
        assert_eq!(policies[1].filter, crate::content_filter::ContentFilterKind::TestsOnly);
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

    #[test]
    fn test_parse_and_detect_docs_config() {
        let sample = r#"
[project]
name = "emulator"

[docs]
paths = ["docs", "ARCHITECTURE.md", "specs/abi"]
description = "RISC-V architecture specifications and ISA manuals"
"#;
        let parsed = parse_project_toml(sample).unwrap();
        assert_eq!(
            parsed.docs.paths,
            vec!["docs", "ARCHITECTURE.md", "specs/abi"]
        );
        assert_eq!(
            parsed.docs.description.as_deref(),
            Some("RISC-V architecture specifications and ISA manuals")
        );
        assert_eq!(parsed.docs.display_summary(), "docs, ARCHITECTURE.md, specs/abi");

        // Compact top-level docs declaration
        let compact = r#"
docs = ["specs", "ARCHITECTURE.md"]
"#;
        let parsed_compact = parse_project_toml(compact).unwrap();
        assert_eq!(
            parsed_compact.docs.paths,
            vec!["specs", "ARCHITECTURE.md"]
        );

        // Auto-detection on filesystem
        let temp = std::env::temp_dir().join(format!("agy_docs_detect_{}", std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).unwrap().as_nanos()));
        std::fs::create_dir_all(temp.join("docs")).unwrap();
        std::fs::create_dir_all(temp.join("specs")).unwrap();
        std::fs::write(temp.join("ARCHITECTURE.md"), "# Architecture\n").unwrap();

        let detected = ProjectConfig::detect_default_docs_paths(&temp);
        assert!(detected.contains(&"docs".to_owned()));
        assert!(detected.contains(&"specs".to_owned()));
        assert!(detected.contains(&"ARCHITECTURE.md".to_owned()));

        let cfg = ProjectConfig::detect_or_load(&temp);
        assert!(cfg.docs.paths.contains(&"docs".to_owned()));
        assert!(cfg.docs.paths.contains(&"ARCHITECTURE.md".to_owned()));

        drop(std::fs::remove_dir_all(&temp));
    }
}
