//! # project
//!
//! Declarative `project.toml` configuration model, parser, and ecosystem auto-detection.
//! Decouples language-specific build tools, compilers, test runners, and build caches
//! from the `agy-suite` core engine.

use std::{
    collections::BTreeMap,
    path::{Path, PathBuf},
};

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
        !self
            .disabled_categories
            .iter()
            .any(|c| c.eq_ignore_ascii_case(name))
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
            self.disabled_categories
                .retain(|c| !c.eq_ignore_ascii_case(name));
        } else if !self
            .disabled_categories
            .iter()
            .any(|c| c.eq_ignore_ascii_case(name))
        {
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
                if cfg.project.language.is_none() {
                    let inferred = Self::infer_from_workspace(workspace_root);
                    cfg.project.language = inferred.project.language;
                }
                return cfg;
            }
        }

        let mut cfg = Self::infer_from_workspace(workspace_root);
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
            gates: BTreeMap::new(),
            sandbox: ProjectSandboxConfig {
                bind_mounts: vec!["target".into()],
            },
            environment: BTreeMap::new(),
            security: ProjectSecurityConfig::default(),
            extra_banned_files: Vec::new(),
            docs: ProjectDocsConfig::default(),
            ..Default::default()
        }
    }

    fn infer_zig() -> Self {
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
            gates: BTreeMap::new(),
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
            gates: BTreeMap::new(),
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
            gates: BTreeMap::new(),
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
        Self {
            project: ProjectMeta {
                name: None,
                language: Some("python".into()),
                version: None,
            },
            workspace: ProjectWorkspaceConfig {
                containers: Vec::new(),
                ignored_paths: vec![
                    ".pytest_cache".into(),
                    ".mypy_cache".into(),
                    "__pycache__".into(),
                ],
            },
            gates: BTreeMap::new(),
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
            gates: BTreeMap::new(),
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
            gates: BTreeMap::new(),
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
/// Uses the official `toml` parser crate for robust syntax and structure deserialization.
///
/// # Errors
/// Returns an error message if the content fails syntax validation.
pub fn parse_project_toml(content: &str) -> Result<ProjectConfig, String> {
    let table: toml::Table = toml::from_str(content).map_err(|err| err.to_string())?;
    let mut config = ProjectConfig::default();

    for (section, val) in &table {
        match section.as_str() {
            "project" => parse_project_section(&mut config, val),
            "workspace" => parse_workspace_section(&mut config, val),
            "gates" => parse_gates_section(&mut config, val),
            "sandbox" => parse_sandbox_section(&mut config, val),
            "environment" => parse_environment_section(&mut config, val),
            "security" => parse_security_section(&mut config, val),
            "extra_banned_files" => {
                if let toml::Value::Array(arr) = val {
                    let items = extract_string_array(arr);
                    config.security.extra_banned_patterns.extend(items.clone());
                    config.extra_banned_files = items;
                }
            },
            "docs" | "docs_paths" => parse_docs_section(&mut config, val),
            "policies" => parse_policies_section(&mut config, val),
            "path_policies" => parse_path_policies_section(&mut config, val),
            _ => {},
        }
    }

    Ok(config)
}

fn extract_string_array(arr: &[toml::Value]) -> Vec<String> {
    arr.iter()
        .filter_map(|x| x.as_str().map(ToOwned::to_owned))
        .collect()
}

fn parse_project_section(config: &mut ProjectConfig, val: &toml::Value) {
    if let toml::Value::Table(tbl) = val {
        for (k, v) in tbl {
            if let Some(s) = v.as_str() {
                match k.as_str() {
                    "name" => config.project.name = Some(s.to_owned()),
                    "language" => config.project.language = Some(s.to_owned()),
                    "version" => config.project.version = Some(s.to_owned()),
                    "docs" | "docs_paths" => config.docs.paths = vec![s.to_owned()],
                    _ => {},
                }
            } else if let toml::Value::Array(arr) = v
                && (k == "docs" || k == "docs_paths" || k == "paths")
            {
                config.docs.paths = extract_string_array(arr);
            }
        }
    }
}

fn parse_workspace_section(config: &mut ProjectConfig, val: &toml::Value) {
    if let toml::Value::Table(tbl) = val {
        for (k, v) in tbl {
            if let toml::Value::Array(arr) = v {
                let strings = extract_string_array(arr);
                match k.as_str() {
                    "containers" => config.workspace.containers = strings,
                    "ignored_paths" => config.workspace.ignored_paths = strings,
                    _ => {},
                }
            }
        }
    }
}

fn parse_gates_section(config: &mut ProjectConfig, val: &toml::Value) {
    if let toml::Value::Table(tbl) = val {
        for (gate_name, gate_val) in tbl {
            if let toml::Value::Table(gate_tbl) = gate_val {
                let mut gate = GateDefinition {
                    command: Vec::new(),
                    timeout_seconds: DEFAULT_GATE_TIMEOUT_SECS,
                    failure_message: None,
                    error_regex: None,
                    explain_command: None,
                };
                for (gk, gv) in gate_tbl {
                    match gk.as_str() {
                        "command" => {
                            if let toml::Value::Array(arr) = gv {
                                gate.command = extract_string_array(arr);
                            }
                        },
                        "timeout_seconds" => {
                            if let Some(n) = gv.as_integer()
                                && let Ok(u) = u64::try_from(n)
                            {
                                gate.timeout_seconds = u;
                            }
                        },
                        "failure_message" => {
                            gate.failure_message = gv.as_str().map(ToOwned::to_owned);
                        },
                        "error_regex" => {
                            gate.error_regex = gv.as_str().map(ToOwned::to_owned);
                        },
                        "explain_command" => {
                            if let toml::Value::Array(arr) = gv {
                                gate.explain_command = Some(extract_string_array(arr));
                            }
                        },
                        _ => {},
                    }
                }
                config.gates.insert(gate_name.clone(), gate);
            }
        }
    }
}

fn parse_sandbox_section(config: &mut ProjectConfig, val: &toml::Value) {
    if let toml::Value::Table(tbl) = val
        && let Some(toml::Value::Array(arr)) = tbl.get("bind_mounts")
    {
        config.sandbox.bind_mounts = extract_string_array(arr);
    }
}

fn parse_environment_section(config: &mut ProjectConfig, val: &toml::Value) {
    if let toml::Value::Table(tbl) = val {
        for (k, v) in tbl {
            if k == "extra_banned_files" || k == "extra_banned_patterns" {
                if let toml::Value::Array(arr) = v {
                    let items = extract_string_array(arr);
                    config.security.extra_banned_patterns.extend(items.clone());
                    config.extra_banned_files = items;
                }
            } else if let Some(s) = v.as_str() {
                config.environment.insert(k.clone(), s.to_owned());
            }
        }
    }
}

fn parse_security_section(config: &mut ProjectConfig, val: &toml::Value) {
    if let toml::Value::Table(tbl) = val {
        for (k, v) in tbl {
            match (k.as_str(), v) {
                ("block_generic_credentials", toml::Value::Boolean(b)) => {
                    config
                        .security
                        .set_category_blocked(SecurityCategory::GenericCredentials, *b);
                },
                ("block_vcs_internals", toml::Value::Boolean(b)) => {
                    config
                        .security
                        .set_category_blocked(SecurityCategory::VcsInternals, *b);
                },
                ("block_ssh_keys", toml::Value::Boolean(b)) => {
                    config
                        .security
                        .set_category_blocked(SecurityCategory::SshKeys, *b);
                },
                ("block_shell_profiles", toml::Value::Boolean(b)) => {
                    config
                        .security
                        .set_category_blocked(SecurityCategory::ShellProfiles, *b);
                },
                ("block_sensitive_extensions", toml::Value::Boolean(b)) => {
                    config
                        .security
                        .set_category_blocked(SecurityCategory::SensitiveExtensions, *b);
                },
                ("extra_banned_patterns" | "extra_banned_files", toml::Value::Array(arr)) => {
                    config.security.extra_banned_patterns = extract_string_array(arr);
                },
                ("allowed_patterns", toml::Value::Array(arr)) => {
                    config.security.allowed_patterns = extract_string_array(arr);
                },
                ("disabled_categories", toml::Value::Array(arr)) => {
                    config.security.disabled_categories = extract_string_array(arr);
                },
                _ => {},
            }
        }
    }
}

fn parse_docs_section(config: &mut ProjectConfig, val: &toml::Value) {
    match val {
        toml::Value::Table(tbl) => {
            for (k, v) in tbl {
                match (k.as_str(), v) {
                    ("description", toml::Value::String(s)) => {
                        config.docs.description = Some(s.clone());
                    },
                    ("path", toml::Value::String(s)) => config.docs.paths = vec![s.clone()],
                    ("paths" | "dirs", toml::Value::Array(arr)) => {
                        config.docs.paths = extract_string_array(arr);
                    },
                    ("tool_name", toml::Value::String(s)) => {
                        config.docs.tool_name = Some(s.clone());
                    },
                    _ => {},
                }
            }
        },
        toml::Value::Array(arr) => {
            config.docs.paths = extract_string_array(arr);
        },
        toml::Value::String(s) => {
            config.docs.paths = vec![s.clone()];
        },
        _ => {},
    }
}

fn parse_policies_section(config: &mut ProjectConfig, val: &toml::Value) {
    if let toml::Value::Table(tbl) = val
        && let Some(toml::Value::Table(syntax_tbl)) = tbl.get("syntax")
    {
        let mut syntax = crate::content_filter::SyntaxConfig::default();
        for (sk, sv) in syntax_tbl {
            if let toml::Value::Array(arr) = sv {
                let strings = extract_string_array(arr);
                match sk.as_str() {
                    "line_comments" => syntax.line_comments = strings,
                    "test_attributes" => syntax.test_attributes = strings,
                    "test_prefixes" => syntax.test_prefixes = strings,
                    _ => {},
                }
            } else if let toml::Value::String(s) = sv {
                match sk.as_str() {
                    "line_comment" => syntax.line_comments = vec![s.clone()],
                    "test_attribute" => syntax.test_attributes = vec![s.clone()],
                    "test_prefix" => syntax.test_prefixes = vec![s.clone()],
                    _ => {},
                }
            }
        }
        config.policies.syntax = Some(syntax);
    }
}

fn parse_path_policies_section(config: &mut ProjectConfig, val: &toml::Value) {
    if let toml::Value::Array(arr) = val {
        for item in arr {
            if let toml::Value::Table(p_tbl) = item {
                let mut policy = crate::content_filter::PathPolicy::default();
                for (pk, pv) in p_tbl {
                    match (pk.as_str(), pv) {
                        ("path", toml::Value::String(s)) => policy.path.clone_from(s),
                        ("filter", toml::Value::String(s)) => match s.as_str() {
                            "comments_only" | "comments" => {
                                policy.filter =
                                    crate::content_filter::ContentFilterKind::CommentsOnly;
                            },
                            "tests_only" | "tests" => {
                                policy.filter = crate::content_filter::ContentFilterKind::TestsOnly;
                            },
                            "delimited" => {
                                policy.filter = crate::content_filter::ContentFilterKind::Delimited;
                            },
                            _ => {},
                        },
                        ("start_marker", toml::Value::String(s)) => {
                            policy.start_marker = Some(s.clone());
                        },
                        ("end_marker", toml::Value::String(s)) => {
                            policy.end_marker = Some(s.clone());
                        },
                        ("line_comment", toml::Value::String(s)) => {
                            policy.line_comments = vec![s.clone()];
                        },
                        ("block_comment_start", toml::Value::String(s)) => {
                            if let Some(bc) = policy.block_comments.last_mut() {
                                s.clone_into(&mut bc.start);
                            } else {
                                policy.block_comments.push(
                                    crate::content_filter::BlockCommentDelimiters::new(
                                        s.as_str(),
                                        "",
                                    ),
                                );
                            }
                        },
                        ("block_comment_end", toml::Value::String(s)) => {
                            if let Some(bc) = policy.block_comments.last_mut() {
                                s.clone_into(&mut bc.end);
                            } else {
                                policy.block_comments.push(
                                    crate::content_filter::BlockCommentDelimiters::new(
                                        "",
                                        s.as_str(),
                                    ),
                                );
                            }
                        },
                        ("line_comments", toml::Value::Array(p_arr)) => {
                            policy.line_comments = extract_string_array(p_arr);
                        },
                        ("test_attributes", toml::Value::Array(p_arr)) => {
                            policy.test_attributes = extract_string_array(p_arr);
                        },
                        ("test_prefixes", toml::Value::Array(p_arr)) => {
                            policy.test_prefixes = extract_string_array(p_arr);
                        },
                        _ => {},
                    }
                }
                config.path_policies.push(policy);
            }
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
            parsed
                .environment
                .get("CARGO_TARGET_DIR")
                .map(String::as_str),
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
path = "src"
filter = "comments_only"
block_comment_start = '"""'
block_comment_end = '"""'

[[path_policies]]
path = "tests"
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
        assert_eq!(policies[0].path, "src");
        assert_eq!(
            policies[0].filter,
            crate::content_filter::ContentFilterKind::CommentsOnly
        );
        assert_eq!(policies[0].block_comments.len(), 1);
        assert_eq!(policies[0].block_comments[0].start, "\"\"\"");
        assert_eq!(policies[0].block_comments[0].end, "\"\"\"");

        assert_eq!(policies[1].path, "tests");
        assert_eq!(
            policies[1].filter,
            crate::content_filter::ContentFilterKind::TestsOnly
        );
    }

    #[test]
    fn test_infer_from_workspace_rust() {
        let temp = std::env::temp_dir().join(format!(
            "agy_proj_test_{}",
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        std::fs::create_dir_all(&temp).unwrap();
        std::fs::write(temp.join("Cargo.toml"), "[package]\nname = \"dummy\"\n").unwrap();

        let cfg = ProjectConfig::infer_from_workspace(&temp);
        assert_eq!(cfg.project.language.as_deref(), Some("rust"));
        assert!(cfg.gates.is_empty());
        assert_eq!(cfg.sandbox.bind_mounts, vec!["target"]);

        drop(std::fs::remove_dir_all(&temp));
    }

    #[test]
    fn test_infer_from_workspace_zig() {
        let temp = std::env::temp_dir().join(format!(
            "agy_zig_test_{}",
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        std::fs::create_dir_all(&temp).unwrap();
        std::fs::write(temp.join("build.zig"), "const std = @import(\"std\");\n").unwrap();

        let cfg = ProjectConfig::infer_from_workspace(&temp);
        assert_eq!(cfg.project.language.as_deref(), Some("zig"));
        assert!(cfg.gates.is_empty());
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
        assert_eq!(
            parsed.docs.display_summary(),
            "docs, ARCHITECTURE.md, specs/abi"
        );

        // Compact top-level docs declaration
        let compact = r#"
docs = ["specs", "ARCHITECTURE.md"]
"#;
        let parsed_compact = parse_project_toml(compact).unwrap();
        assert_eq!(parsed_compact.docs.paths, vec!["specs", "ARCHITECTURE.md"]);

        // Auto-detection on filesystem
        let temp = std::env::temp_dir().join(format!(
            "agy_docs_detect_{}",
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
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
