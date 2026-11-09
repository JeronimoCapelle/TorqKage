//! Composable security guards, path confinement, and UTF-8 truncation primitives.

use std::{
    collections::BTreeMap,
    path::{Component, Path, PathBuf},
};

use tkg_common::{GateCheck, ProjectSecurityConfig, matches_directory_scope_with_containers};

use crate::models::{AgentGuard, ToolDefinition};

/// Inspects a target path against dangerous files, secrets, and repository internals,
/// guided by optional per-project security configuration.
///
/// # Errors
/// Returns an error if the path attempts to access sensitive files, credentials, or VCS internals (`.git/`, `.jj/`),
/// unless explicitly exempted in `security.allowed_patterns`.
pub(crate) fn check_dangerous_files_with_security(
    path: &Path,
    security: Option<&ProjectSecurityConfig>,
) -> Result<(), String> {
    tkg_common::check_dangerous_files_with_security(path, security)
}

/// Resolves and confines a relative path within the canonical workspace bounds.
///
/// # Errors
/// Returns an error if the path escapes workspace bounds, traverses `..`, or violates directory allowlists.
pub(crate) fn resolve_safe_path(
    workspace_root: &Path,
    rel_path_str: &str,
    allowed_directories: &[String],
    enforce_bounds: bool,
) -> Result<PathBuf, String> {
    let clean_str = rel_path_str.trim();
    if clean_str.is_empty() {
        return Err("Path argument cannot be empty".to_owned());
    }

    let input_path = Path::new(clean_str);

    // Reject any raw parent component traversal attempts
    for component in input_path.components() {
        if matches!(component, Component::ParentDir) {
            return Err(
                "Security policy violation: Path traversal via '..' is forbidden".to_owned(),
            );
        }
    }

    let candidate = if input_path.is_absolute() {
        input_path.to_path_buf()
    } else {
        workspace_root.join(input_path)
    };

    if enforce_bounds {
        // If file exists, check canonical root prefix
        if candidate.exists() {
            let canonical = candidate.canonicalize().map_err(|err| {
                format!(
                    "Failed to canonicalize path '{}': {err}",
                    candidate.display()
                )
            })?;

            let canonical_root = workspace_root
                .canonicalize()
                .unwrap_or_else(|_| workspace_root.to_path_buf());

            if !canonical.starts_with(&canonical_root) {
                return Err(format!(
                    "Security policy violation: Path '{}' escapes canonical workspace root '{}'",
                    canonical.display(),
                    canonical_root.display()
                ));
            }
        } else {
            // For not-yet-existing files, verify parent
            if let Some(parent) = candidate.parent()
                && parent.exists()
            {
                let canonical_parent = parent.canonicalize().map_err(|err| {
                    format!(
                        "Failed to canonicalize parent '{}': {err}",
                        parent.display()
                    )
                })?;
                let canonical_root = workspace_root
                    .canonicalize()
                    .unwrap_or_else(|_| workspace_root.to_path_buf());

                if !canonical_parent.starts_with(&canonical_root) {
                    return Err(
                        "Security policy violation: Path parent escapes workspace bounds"
                            .to_owned(),
                    );
                }
            }
        }
    }

    // Check allowed directories whitelist
    if !allowed_directories.is_empty() {
        let relative = candidate.strip_prefix(workspace_root).unwrap_or(input_path);

        let rel_lossy = relative.to_string_lossy();
        let project = tkg_common::ProjectConfig::detect_or_load(workspace_root);
        let containers = project.resolved_containers(workspace_root);
        let allowed = allowed_directories
            .iter()
            .any(|dir| matches_directory_scope_with_containers(&rel_lossy, dir, &containers));

        if !allowed {
            return Err(format!(
                "Security policy violation: Path '{rel_lossy}' is outside allowed directories: {allowed_directories:?}"
            ));
        }
    }

    Ok(candidate)
}

/// Enforces single-directory confinement (`enforce_dir`) and single-file target locking (`enforce_file_target`).
///
/// # Errors
/// Returns an error if the candidate path is outside `enforce_dir` or does not match `enforce_file_target`.
pub(crate) fn check_enforce_boundaries(
    workspace_root: &Path,
    candidate: &Path,
    enforce_dir: Option<&str>,
    enforce_file_target: Option<&str>,
) -> Result<(), String> {
    let relative = candidate.strip_prefix(workspace_root).unwrap_or(candidate);
    let rel_lossy = relative.to_string_lossy();

    // 1. Enforce single-file target locking
    if let Some(target) = enforce_file_target {
        let expected_rel = Path::new(target)
            .strip_prefix(workspace_root)
            .unwrap_or_else(|_| Path::new(target));
        let expected_lossy = expected_rel.to_string_lossy();

        if rel_lossy != expected_lossy {
            return Err(format!(
                "Security policy violation: Tool is strictly locked to target file '{expected_lossy}', but '{rel_lossy}' was requested."
            ));
        }
    }

    // 2. Enforce single-directory confinement
    if let Some(dir) = enforce_dir {
        let project = tkg_common::ProjectConfig::detect_or_load(workspace_root);
        let containers = project.resolved_containers(workspace_root);
        if !matches_directory_scope_with_containers(&rel_lossy, dir, &containers) {
            return Err(format!(
                "Security policy violation: Path '{rel_lossy}' is outside enforced directory '{dir}'"
            ));
        }
    }

    Ok(())
}

/// Enforces agent-specific directory and file path boundaries.
///
/// # Errors
/// Returns an error if the agent is restricted and the target path violates the agent's guard.
pub(crate) fn check_agent_guard(
    workspace_root: &Path,
    candidate: &Path,
    agent_name: Option<&str>,
    guards: &BTreeMap<String, AgentGuard>,
) -> Result<(), String> {
    let guard = agent_name.and_then(|a| guards.get(a));
    tkg_common::check_agent_guard_direct(workspace_root, candidate, agent_name, guard)
}

/// Enforces declarative agent path policies (e.g. `comments_only`, `tests_only`, `delimited`).
///
/// Supports language-agnostic syntax resolution via `project.toml` and auto-detection.
///
/// # Errors
/// Returns an error if the agent attempts to modify forbidden content within a policy-guarded path.
pub(crate) fn check_agent_path_policies(
    workspace_root: &Path,
    candidate: &Path,
    original: &str,
    new_content: &str,
    agent_name: Option<&str>,
    guards: &BTreeMap<String, AgentGuard>,
    project: Option<&tkg_common::ProjectConfig>,
) -> Result<(), String> {
    let guard = agent_name.and_then(|a| guards.get(a));
    tkg_common::check_agent_path_policies_direct(
        workspace_root,
        candidate,
        original,
        new_content,
        agent_name,
        guard,
        project,
    )
}

/// Verifies whether the workspace contains unresolved VCS conflict markers.
///
/// # Errors
/// Returns an error if conflict markers (e.g. `<<<<<<<`) are detected in the repository.
pub(crate) fn check_vcs_conflicts(workspace_root: &Path) -> Result<(), String> {
    match tkg_common::VcsConflictCheck.execute(workspace_root) {
        tkg_common::GateCheckStatus::Passed => Ok(()),
        tkg_common::GateCheckStatus::Failed(reason) => Err(format!(
            "Security gate blocked mutation: Unresolved merge conflicts detected: {reason}"
        )),
        tkg_common::GateCheckStatus::Warning(warn) => Err(format!(
            "Security gate blocked mutation: Potential merge conflicts detected: {warn}"
        )),
    }
}

/// Verifies that unauthorized agents do not mutate protected build manifests.
///
/// Neutralizes procedural-macro / `build.rs` execution sandbox escape vectors.
///
/// Delegates to canonical [`tkg_common::manifest_guard::check_manifest_protection`].
///
/// # Errors
/// Returns an error if the agent is not permitted to mutate protected build files.
pub(crate) fn check_manifest_protection(
    target_path: &Path,
    agent_name: Option<&str>,
    guard: Option<&AgentGuard>,
) -> Result<(), String> {
    tkg_common::manifest_guard::check_manifest_protection(target_path, agent_name, guard)
}

const COMMON_ROOT_DIRS: &[&str] = &[
    "src",
    "lib",
    "pkg",
    "app",
    "cmd",
    "tests",
    "test",
    "spec",
    "benches",
    "examples",
    "docs",
    "doc",
    "include",
    "scripts",
    "bin",
    ".git",
    ".jj",
    ".tkg",
    ".opencode",
    ".claude",
];

/// Helper attempting to resolve a scoped path within declared monorepo subcrate containers.
fn resolve_in_subcrates(
    workspace_root: &Path,
    clean: &str,
    scopes: &[&str],
    containers: &[String],
) -> Option<String> {
    if containers.is_empty() {
        return None;
    }

    // Case A: Path starts with "<container>/<crate_name>/..."
    for container in containers {
        let prefix = format!("{container}/");
        if let Some(sub) = clean.strip_prefix(&prefix)
            && let Some((crate_name, rest)) = sub.split_once('/')
        {
            for scope in scopes {
                let candidate = workspace_root
                    .join(container)
                    .join(crate_name)
                    .join(scope)
                    .join(rest);
                if candidate.exists() {
                    return Some(format!("{container}/{crate_name}/{scope}/{rest}"));
                }
            }
        }
    }

    // Case B: Path starts with "<crate_name>/<rest>" where <crate_name> exists in a container
    if let Some((possible_crate, rest)) = clean.split_once('/') {
        for container in containers {
            let crate_dir = workspace_root.join(container).join(possible_crate);
            if crate_dir.is_dir() {
                for scope in scopes {
                    let candidate = crate_dir.join(scope).join(rest);
                    if candidate.exists() {
                        return Some(format!("{container}/{possible_crate}/{scope}/{rest}"));
                    }
                }
            }
        }
    }

    // Case C: Path doesn't mention crate name (e.g. "actions/file_read.rs" or "guards.rs")
    // Find unique match across all crates in all containers
    let mut unique_match: Option<String> = None;
    let mut match_count = 0_usize;

    for container in containers {
        let container_dir = workspace_root.join(container);
        let Ok(read_dir) = std::fs::read_dir(container_dir) else {
            continue;
        };

        for entry in read_dir.flatten() {
            let entry_path = entry.path();
            if !entry_path.is_dir() {
                continue;
            }
            let crate_name = entry.file_name().to_string_lossy().to_string();

            for scope in scopes {
                let candidate = entry_path.join(scope).join(clean);
                if candidate.exists() {
                    match_count = match_count.saturating_add(1);
                    unique_match = Some(format!("{container}/{crate_name}/{scope}/{clean}"));
                }
            }
        }
    }

    if match_count == 1 { unique_match } else { None }
}

fn collect_candidate_scopes(
    tool: Option<&ToolDefinition>,
    agent_context: Option<(&str, &BTreeMap<String, AgentGuard>)>,
) -> Vec<String> {
    let mut scopes = Vec::new();

    if let Some(tool_def) = tool
        && let Some(ref enforce_dir) = tool_def.enforce_dir
    {
        let s = enforce_dir.trim_matches(['/', '\\']).to_owned();
        if !s.is_empty() && s != "*" && !scopes.contains(&s) {
            scopes.push(s);
        }
    }

    if let Some((agent_name, guards)) = agent_context
        && let Some(guard) = guards.get(agent_name)
    {
        for dir in &guard.allowed_dirs {
            let s = dir.trim_matches(['/', '\\']).to_owned();
            if !s.is_empty() && s != "*" && !scopes.contains(&s) {
                scopes.push(s);
            }
        }
    }

    if let Some(tool_def) = tool {
        for dir in &tool_def.allowed_directories {
            let s = dir.trim_matches(['/', '\\']).to_owned();
            if !s.is_empty() && s != "*" && !scopes.contains(&s) {
                scopes.push(s);
            }
        }
    }

    scopes
}

fn anchor_to_primary_scope(
    workspace_root: &Path,
    clean: &str,
    primary_scope: &str,
    containers: &[String],
) -> String {
    for container in containers {
        let prefix = format!("{container}/");
        if let Some(sub) = clean.strip_prefix(&prefix)
            && let Some((crate_name, rest)) = sub.split_once('/')
            && workspace_root.join(container).join(crate_name).is_dir()
        {
            return format!("{container}/{crate_name}/{primary_scope}/{rest}");
        }
    }

    if let Some((possible_crate, rest)) = clean.split_once('/') {
        for container in containers {
            if workspace_root.join(container).join(possible_crate).is_dir() {
                return format!("{container}/{possible_crate}/{primary_scope}/{rest}");
            }
        }
    }

    format!("{primary_scope}/{clean}")
}

/// Resolves a path string relative to agent and tool scopes.
///
/// If an agent or tool is confined to a specific directory (e.g., `src` or `docs`), this resolves
/// relative paths (e.g., `"auth/hasher.rs"` or `"specs/rv32.md"`) to their scoped locations
/// without forcing the caller to supply the parent directory prefix.
///
/// Existing workspace paths, absolute paths, and explicit allowed files are preserved transparently.
#[must_use]
pub(crate) fn resolve_scoped_path_str(
    workspace_root: &Path,
    rel_path_str: &str,
    tool: Option<&ToolDefinition>,
    agent_context: Option<(&str, &BTreeMap<String, AgentGuard>)>,
) -> String {
    let raw_norm = rel_path_str.trim().replace('\\', "/");
    let mut clean = raw_norm.as_str();
    while let Some(stripped) = clean.strip_prefix("./") {
        clean = stripped.trim();
    }

    if clean.is_empty() || Path::new(clean).is_absolute() {
        return clean.to_owned();
    }

    // 0. If tool has enforce_file_target, check if clean matches target or its filename
    if let Some(target) = tool.and_then(|t| t.enforce_file_target.as_deref()) {
        let clean_target = target.trim_matches(['/', '\\']).replace('\\', "/");
        if clean == clean_target {
            return clean_target;
        }
        if let Some(fname) = Path::new(&clean_target)
            .file_name()
            .and_then(|f| f.to_str())
            && clean == fname
        {
            return clean_target;
        }
    }

    // 1. If explicit allowed file for this agent, leave as-is
    if let Some((agent_name, guards)) = agent_context
        && let Some(guard) = guards.get(agent_name)
        && guard.allowed_files.iter().any(|f| f == clean)
    {
        return clean.to_owned();
    }

    // 2. Collect candidate scopes (ordered: tool enforce_dir, agent allowed_dirs, tool allowed_directories)
    let scopes = collect_candidate_scopes(tool, agent_context);
    if scopes.is_empty() {
        return clean.to_owned();
    }

    let scope_refs: Vec<&str> = scopes.iter().map(String::as_str).collect();

    let project = tkg_common::ProjectConfig::detect_or_load(workspace_root);
    let containers = project.resolved_containers(workspace_root);

    // 3. If the path ALREADY matches any candidate scope, don't double-prefix it!
    for scope in &scope_refs {
        if matches_directory_scope_with_containers(clean, scope, &containers) {
            return clean.to_owned();
        }
    }

    // 4. If the agent explicitly attempts to access one of its disallowed directories,
    // do NOT anchor it into an allowed scope. Leave it intact so security checks will reject it.
    if let Some((agent_name, guards)) = agent_context
        && let Some(guard) = guards.get(agent_name)
        && guard
            .disallowed_dirs
            .iter()
            .any(|d| matches_directory_scope_with_containers(clean, d, &containers))
    {
        return clean.to_owned();
    }

    // 5. If the path already exists directly at workspace root, leave as-is (e.g. cross-directory read)
    if workspace_root.join(clean).exists() {
        return clean.to_owned();
    }

    // 6. Check if the path exists inside candidate scope directly (e.g. "src/auth/hasher.rs")
    for scope in &scope_refs {
        let candidate = workspace_root.join(scope).join(clean);
        if candidate.exists() {
            return format!("{scope}/{clean}");
        }
    }

    // 7. Check workspace multi-crate structure using declared containers
    if let Some(resolved) = resolve_in_subcrates(workspace_root, clean, &scope_refs, &containers) {
        return resolved;
    }

    // 8. If clean starts with a known top-level project directory that is NOT one of our scopes,
    // do NOT anchor it into our scope (e.g. "tests/test.rs" when scoped to "src").
    if let Some((first_component, _)) = clean.split_once('/')
        && !containers.iter().any(|c| c == first_component)
    {
        let is_known_root = COMMON_ROOT_DIRS.contains(&first_component)
            || project.docs.paths.iter().any(|d| d == first_component)
            || workspace_root.join(first_component).is_dir();
        if is_known_root && !scope_refs.contains(&first_component) {
            return clean.to_owned();
        }
    }

    // 9. If file does not exist yet (e.g. writing a new file via file_write), anchor to primary scope
    let primary = tool
        .and_then(|t| t.enforce_dir.as_deref())
        .map(|s| s.trim_matches(['/', '\\']))
        .filter(|s| !s.is_empty() && *s != "*")
        .or_else(|| scope_refs.first().copied());

    if let Some(primary_scope) = primary {
        return anchor_to_primary_scope(workspace_root, clean, primary_scope, &containers);
    }

    clean.to_owned()
}

/// Truncates string output by lines and bytes safely on UTF-8 boundaries.
pub(crate) fn truncate_output(
    text: &str,
    max_lines: Option<usize>,
    max_bytes: Option<usize>,
    emit_notice: bool,
) -> String {
    let total_lines = text.lines().count();
    let total_bytes = text.len();

    let mut truncated = false;

    // 1. Line-based truncation
    let mut truncated_text = if let Some(limit_lines) = max_lines
        && total_lines > limit_lines
    {
        truncated = true;
        let lines: Vec<&str> = text.lines().take(limit_lines).collect();
        lines.join("\n")
    } else {
        text.to_owned()
    };

    // 2. Byte-based truncation with safe UTF-8 character boundary slicing
    if let Some(limit_bytes) = max_bytes
        && truncated_text.len() > limit_bytes
    {
        truncated = true;
        // Floor to nearest valid char boundary <= limit_bytes
        let mut boundary = limit_bytes;
        while boundary > 0 && !truncated_text.is_char_boundary(boundary) {
            boundary = boundary.saturating_sub(1);
        }

        if let Some(slice) = truncated_text.get(..boundary) {
            truncated_text = slice.to_owned();
        }
    }

    if truncated && emit_notice {
        let notice = format!(
            "\n\n[TRUNCATION NOTICE: Output truncated. Showing {} of {} lines, {} of {} bytes]",
            truncated_text.lines().count(),
            total_lines,
            truncated_text.len(),
            total_bytes
        );
        truncated_text.push_str(&notice);
    }

    truncated_text
}

/// Strips raw ANSI terminal styling sequences (e.g. colors, bold, cursor codes).
pub(crate) fn strip_ansi(input: &str) -> String {
    anstream::adapter::strip_str(input).to_string()
}

#[cfg(test)]
#[allow(
    clippy::expect_used,
    clippy::unwrap_used,
    clippy::panic,
    clippy::indexing_slicing,
    clippy::assertions_on_result_states,
    reason = "Unit test assertions"
)]
mod tests {
    use tkg_common::path_matches_pattern;

    use super::*;

    fn check_dangerous_files(path: &Path) -> Result<(), String> {
        check_dangerous_files_with_security(path, None)
    }

    #[test]
    fn test_dangerous_files_rejected() {
        assert!(check_dangerous_files(Path::new(".env")).is_err());
        assert!(check_dangerous_files(Path::new("config/.env.local")).is_err());
        assert!(check_dangerous_files(Path::new("keys/id_rsa")).is_err());
        assert!(check_dangerous_files(Path::new(".git/HEAD")).is_err());
        assert!(check_dangerous_files(Path::new(".jj/working_copy")).is_err());
        assert!(check_dangerous_files(Path::new("secrets/private.pem")).is_err());
        assert!(check_dangerous_files(Path::new("src/lib.rs")).is_ok());
        assert!(check_dangerous_files(Path::new("src/tokenizer.rs")).is_ok());
        assert!(check_dangerous_files(Path::new("src/tokens.rs")).is_ok());
    }

    #[test]
    fn test_path_traversal_rejected() {
        let root = std::env::temp_dir();
        assert!(resolve_safe_path(&root, "../etc/passwd", &[], true).is_err());
        assert!(resolve_safe_path(&root, "foo/../../etc", &[], true).is_err());
    }

    #[test]
    fn test_allowed_directories_enforced() {
        let root = std::env::temp_dir().join(format!(
            "tkg_allowed_dir_test_{}",
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        drop(std::fs::create_dir_all(&root));
        drop(std::fs::write(
            root.join("project.toml"),
            "[workspace]\ncontainers = [\"crates\"]\n",
        ));

        let allowed = vec!["tests".to_owned(), "benches".to_owned()];

        assert!(resolve_safe_path(&root, "tests/unit_test.rs", &allowed, false).is_ok());
        assert!(resolve_safe_path(&root, "crates/core/tests/test.rs", &allowed, false).is_ok());
        assert!(resolve_safe_path(&root, "src/main.rs", &allowed, false).is_err());
        assert!(
            resolve_safe_path(&root, "docs/fixtures/tests/sample.rs", &allowed, false).is_err()
        );

        drop(std::fs::remove_dir_all(&root));
    }

    #[test]
    fn test_truncation() {
        let sample = "Line 1\nLine 2\nLine 3\nLine 4\nLine 5";
        let truncated = truncate_output(sample, Some(2), None, true);
        assert!(truncated.contains("Line 1\nLine 2"));
        assert!(truncated.contains("[TRUNCATION NOTICE:"));
    }

    #[test]
    fn test_enforce_boundaries() {
        let root = std::env::temp_dir();

        // Enforce file target
        let good_file = root.join("ARCHITECTURE.md");
        let bad_file = root.join("src/lib.rs");
        assert!(check_enforce_boundaries(&root, &good_file, None, Some("ARCHITECTURE.md")).is_ok());
        assert!(check_enforce_boundaries(&root, &bad_file, None, Some("ARCHITECTURE.md")).is_err());

        // Enforce dir
        let good_dir = root.join("docs/specs/rv32.md");
        let bad_dir = root.join("src/main.rs");
        assert!(check_enforce_boundaries(&root, &good_dir, Some("docs"), None).is_ok());
        assert!(check_enforce_boundaries(&root, &bad_dir, Some("docs"), None).is_err());
    }

    #[test]
    fn test_check_agent_guard() {
        let root = std::env::temp_dir();
        let mut guards = BTreeMap::new();

        guards.insert(
            "worker_agent".to_owned(),
            AgentGuard {
                allowed_dirs: vec!["src".to_owned()],
                disallowed_dirs: vec!["tests".to_owned()],
                allowed_files: Vec::new(),
                disallowed_files: vec!["Cargo.toml".to_owned()],
                ..Default::default()
            },
        );

        guards.insert(
            "docs_agent".to_owned(),
            AgentGuard {
                allowed_dirs: vec!["docs".to_owned()],
                disallowed_dirs: vec!["src".to_owned()],
                allowed_files: vec!["spec.md".to_owned()],
                disallowed_files: Vec::new(),
                ..Default::default()
            },
        );

        // worker_agent tests
        assert!(
            check_agent_guard(
                &root,
                &root.join("src/lib.rs"),
                Some("worker_agent"),
                &guards
            )
            .is_ok()
        );
        assert!(
            check_agent_guard(
                &root,
                &root.join("tests/test.rs"),
                Some("worker_agent"),
                &guards
            )
            .is_err()
        );
        assert!(
            check_agent_guard(
                &root,
                &root.join("Cargo.toml"),
                Some("worker_agent"),
                &guards
            )
            .is_err()
        );

        // docs_agent tests
        assert!(
            check_agent_guard(&root, &root.join("spec.md"), Some("docs_agent"), &guards).is_ok()
        );
        assert!(
            check_agent_guard(
                &root,
                &root.join("docs/rfc.md"),
                Some("docs_agent"),
                &guards
            )
            .is_ok()
        );
        assert!(
            check_agent_guard(&root, &root.join("src/lib.rs"), Some("docs_agent"), &guards)
                .is_err()
        );

        // Unrestricted agent
        assert!(
            check_agent_guard(
                &root,
                &root.join("src/lib.rs"),
                Some("unrestricted_agent"),
                &guards
            )
            .is_ok()
        );
        assert!(check_agent_guard(&root, &root.join("tests/test.rs"), None, &guards).is_ok());
    }

    fn setup_test_guards() -> (PathBuf, BTreeMap<String, AgentGuard>) {
        let root = PathBuf::from("/workspace");
        let mut guards = BTreeMap::new();

        guards.insert(
            "comments_guarded_agent".to_owned(),
            AgentGuard {
                path_policies: vec![tkg_common::PathPolicy::comments_only("src")],
                ..Default::default()
            },
        );

        guards.insert(
            "tests_guarded_agent".to_owned(),
            AgentGuard {
                path_policies: vec![tkg_common::PathPolicy {
                    test_attributes: vec!["test".to_owned(), "cfg(test".to_owned()],
                    ..tkg_common::PathPolicy::tests_only("src")
                }],
                ..Default::default()
            },
        );

        (root, guards)
    }

    #[test]
    fn test_check_agent_path_policies_comments_only() {
        let (root, guards) = setup_test_guards();
        let orig = "pub fn foo() -> u32 { 1 }";
        let with_docs = "/// Doc comment\npub fn foo() -> u32 {\n    // Inline comment\n    1\n}";
        let with_code_tamper = "pub fn foo() -> u32 { 2 }";

        // comments_guarded_agent: comments allowed, code edits blocked in src/
        assert!(
            check_agent_path_policies(
                &root,
                &root.join("src/lib.rs"),
                orig,
                with_docs,
                Some("comments_guarded_agent"),
                &guards,
                None
            )
            .is_ok()
        );
        assert!(
            check_agent_path_policies(
                &root,
                &root.join("src/lib.rs"),
                orig,
                with_code_tamper,
                Some("comments_guarded_agent"),
                &guards,
                None
            )
            .is_err()
        );
    }

    #[test]
    fn test_check_agent_path_policies_tests_only() {
        let (root, guards) = setup_test_guards();
        let orig = "pub fn foo() -> u32 { 1 }";
        let with_code_tamper = "pub fn foo() -> u32 { 2 }";

        // tests_guarded_agent: tests allowed in src/, production code edits blocked
        let with_unit_test =
            "pub fn foo() -> u32 { 1 }\n#[cfg(test)]\nmod tests {\n    #[test]\n    fn t() {}\n}";
        assert!(
            check_agent_path_policies(
                &root,
                &root.join("src/lib.rs"),
                orig,
                with_unit_test,
                Some("tests_guarded_agent"),
                &guards,
                None
            )
            .is_ok()
        );
        assert!(
            check_agent_path_policies(
                &root,
                &root.join("src/lib.rs"),
                orig,
                with_code_tamper,
                Some("tests_guarded_agent"),
                &guards,
                None
            )
            .is_err()
        );
    }

    #[test]
    fn test_check_agent_path_policies_unconfigured_agent() {
        let (root, guards) = setup_test_guards();
        let orig = "pub fn foo() -> u32 { 1 }";
        let with_code_tamper = "pub fn foo() -> u32 { 2 }";

        // unconfigured agent: edits allowed
        assert!(
            check_agent_path_policies(
                &root,
                &root.join("src/lib.rs"),
                orig,
                with_code_tamper,
                Some("unconfigured_agent"),
                &guards,
                None
            )
            .is_ok()
        );
    }

    #[test]
    fn test_check_agent_path_policies_declarative_project() {
        let (root, guards) = setup_test_guards();

        // Declarative project.toml custom path policy for Python
        let mut proj = tkg_common::ProjectConfig::default();
        proj.project.language = Some("python".to_owned());
        proj.policies.syntax = Some(tkg_common::SyntaxConfig {
            line_comments: vec!["#".to_owned()],
            block_comments: vec![tkg_common::BlockCommentDelimiters::new("\"\"\"", "\"\"\"")],
            ..Default::default()
        });
        proj.path_policies.push(tkg_common::PathPolicy {
            path: "app".to_owned(),
            filter: tkg_common::ContentFilterKind::CommentsOnly,
            ..Default::default()
        });

        let py_orig = "def add(a, b):\n    return a + b\n";
        let py_docs =
            "def add(a, b):\n    \"\"\"Docstring\"\"\"\n    # comment\n    return a + b\n";
        let py_tamper = "def add(a, b):\n    return a + b + 1\n";

        assert!(
            check_agent_path_policies(
                &root,
                &root.join("app/math.py"),
                py_orig,
                py_docs,
                Some("comments_guarded_agent"),
                &guards,
                Some(&proj)
            )
            .is_ok()
        );
        assert!(
            check_agent_path_policies(
                &root,
                &root.join("app/math.py"),
                py_orig,
                py_tamper,
                Some("comments_guarded_agent"),
                &guards,
                Some(&proj)
            )
            .is_err()
        );
    }

    #[test]
    fn test_path_matches_pattern_wildcards() {
        assert!(path_matches_pattern(
            "src/auth/password/hasher.rs",
            "src/auth/password/**"
        ));
        assert!(path_matches_pattern(
            "src/auth/password",
            "src/auth/password/**"
        ));
        assert!(path_matches_pattern("fixtures/keys/cert.pem", "*.pem"));
        assert!(path_matches_pattern(".env.test", ".env*"));
        assert!(path_matches_pattern("tests/secrets/token.json", "*"));
        assert!(path_matches_pattern(
            "deep/nested/path/firmware.bin",
            "firmware.bin"
        ));
        // Standard globs unlocked by globset
        assert!(path_matches_pattern(
            "src/controllers/user.rs",
            "src/**/*.rs"
        ));
        assert!(path_matches_pattern("secret.key", "*.{key,pem}"));
        assert!(path_matches_pattern("cert.pem", "*.{key,pem}"));
        assert!(!path_matches_pattern("doc.md", "*.{key,pem}"));
    }

    #[test]
    fn test_modular_security_credentials_and_allowlist() {
        let path = Path::new("src/auth/password/mod.rs");
        // By default, generic credential "password" is blocked
        assert!(check_dangerous_files_with_security(path, None).is_err());

        // With block_generic_credentials = false, it is allowed
        let mut sec_perm = ProjectSecurityConfig::default();
        sec_perm.set_category_blocked(tkg_common::SecurityCategory::GenericCredentials, false);
        assert!(check_dangerous_files_with_security(path, Some(&sec_perm)).is_ok());

        // With allowed_patterns, specific directory is allowed even if category blocked
        let sec_allow = ProjectSecurityConfig {
            allowed_patterns: vec!["src/auth/password/**".into()],
            ..Default::default()
        };
        assert!(check_dangerous_files_with_security(path, Some(&sec_allow)).is_ok());

        // But unrelated credentials are still blocked
        assert!(
            check_dangerous_files_with_security(
                Path::new("src/secrets/token.txt"),
                Some(&sec_allow)
            )
            .is_err()
        );

        // With extra_banned_patterns, custom paths are rejected
        let sec_banned = ProjectSecurityConfig {
            extra_banned_patterns: vec!["internal_firmware.bin".into()],
            ..Default::default()
        };
        assert!(
            check_dangerous_files_with_security(
                Path::new("crates/core/internal_firmware.bin"),
                Some(&sec_banned)
            )
            .is_err()
        );
    }

    #[test]
    fn test_resolve_scoped_path_str_single_crate() {
        let root = std::env::temp_dir().join(format!("tkg_scope_test_{}", std::process::id()));
        drop(std::fs::remove_dir_all(&root));
        std::fs::create_dir_all(root.join("src/auth")).expect("create src/auth");
        std::fs::create_dir_all(root.join("docs/specs")).expect("create docs/specs");
        std::fs::write(root.join("Cargo.toml"), "[package]\nname = \"demo\"\n")
            .expect("write root Cargo.toml");
        std::fs::write(root.join("src/auth/hasher.rs"), "// hasher").expect("write hasher.rs");
        std::fs::write(root.join("docs/specs/rv32.md"), "# RV32").expect("write rv32.md");

        let mut guards = BTreeMap::new();
        guards.insert(
            "worker_agent".to_owned(),
            AgentGuard {
                allowed_dirs: vec!["src".to_owned()],
                disallowed_dirs: vec!["tests".to_owned()],
                allowed_files: Vec::new(),
                disallowed_files: vec!["Cargo.toml".to_owned()],
                ..Default::default()
            },
        );
        guards.insert(
            "docs_agent".to_owned(),
            AgentGuard {
                allowed_dirs: vec!["docs".to_owned()],
                disallowed_dirs: vec!["src".to_owned()],
                allowed_files: vec!["spec.md".to_owned()],
                disallowed_files: Vec::new(),
                ..Default::default()
            },
        );

        // 1. worker_agent scoped resolution
        assert_eq!(
            resolve_scoped_path_str(
                &root,
                "auth/hasher.rs",
                None,
                Some(("worker_agent", &guards))
            ),
            "src/auth/hasher.rs"
        );

        // 2. Already prefixed remains unchanged
        assert_eq!(
            resolve_scoped_path_str(
                &root,
                "src/auth/hasher.rs",
                None,
                Some(("worker_agent", &guards))
            ),
            "src/auth/hasher.rs"
        );

        // 3. Leading ./ stripped
        assert_eq!(
            resolve_scoped_path_str(
                &root,
                "./auth/hasher.rs",
                None,
                Some(("worker_agent", &guards))
            ),
            "src/auth/hasher.rs"
        );

        // 4. Absolute path preserved
        assert_eq!(
            resolve_scoped_path_str(
                &root,
                "/tmp/absolute.rs",
                None,
                Some(("worker_agent", &guards))
            ),
            "/tmp/absolute.rs"
        );

        // 5. Existing root file preserved for read/inspection
        assert_eq!(
            resolve_scoped_path_str(&root, "Cargo.toml", None, Some(("worker_agent", &guards))),
            "Cargo.toml"
        );

        // 6. Explicit allowed file for agent preserved
        assert_eq!(
            resolve_scoped_path_str(&root, "spec.md", None, Some(("docs_agent", &guards))),
            "spec.md"
        );

        // 7. Disallowed directory is NOT rewritten into allowed scope (preserves security rejection)
        assert_eq!(
            resolve_scoped_path_str(
                &root,
                "tests/unit_test.rs",
                None,
                Some(("worker_agent", &guards))
            ),
            "tests/unit_test.rs"
        );

        drop(std::fs::remove_dir_all(&root));
    }

    #[test]
    fn test_resolve_scoped_path_str_tools_and_new_files() {
        let root = std::env::temp_dir().join(format!("tkg_scope_tools_{}", std::process::id()));
        drop(std::fs::remove_dir_all(&root));
        std::fs::create_dir_all(root.join("src/auth")).expect("create src/auth");
        std::fs::create_dir_all(root.join("docs/specs")).expect("create docs/specs");
        std::fs::write(root.join("docs/specs/rv32.md"), "# RV32").expect("write rv32.md");

        let mut guards = BTreeMap::new();
        guards.insert(
            "worker_agent".to_owned(),
            AgentGuard {
                allowed_dirs: vec!["src".to_owned()],
                ..Default::default()
            },
        );

        // 8. Tool enforce_dir resolution
        let tool_docs = ToolDefinition {
            name: "read_spec".to_owned(),
            enforce_dir: Some("docs".to_owned()),
            ..Default::default()
        };
        assert_eq!(
            resolve_scoped_path_str(&root, "specs/rv32.md", Some(&tool_docs), None),
            "docs/specs/rv32.md"
        );

        // 9. Tool enforce_file_target resolution
        let tool_arch = ToolDefinition {
            name: "update_arch".to_owned(),
            enforce_file_target: Some("docs/ARCHITECTURE.md".to_owned()),
            ..Default::default()
        };
        assert_eq!(
            resolve_scoped_path_str(&root, "ARCHITECTURE.md", Some(&tool_arch), None),
            "docs/ARCHITECTURE.md"
        );

        // 10. New file creation in scope when file does not exist yet
        assert_eq!(
            resolve_scoped_path_str(&root, "auth/jwt.rs", None, Some(("worker_agent", &guards))),
            "src/auth/jwt.rs"
        );

        drop(std::fs::remove_dir_all(&root));
    }

    #[test]
    fn test_resolve_scoped_path_str_multi_crate() {
        let root = std::env::temp_dir().join(format!("tkg_multi_crate_{}", std::process::id()));
        drop(std::fs::remove_dir_all(&root));
        std::fs::create_dir_all(root.join("crates/core/src")).expect("create crates/core/src");
        std::fs::create_dir_all(root.join("crates/api/src")).expect("create crates/api/src");
        std::fs::write(
            root.join("project.toml"),
            "[workspace]\ncontainers = [\"crates\"]\n",
        )
        .expect("write project.toml");

        std::fs::write(root.join("crates/core/src/hasher.rs"), "// hasher")
            .expect("write hasher.rs");
        std::fs::write(root.join("crates/core/src/lib.rs"), "// core lib")
            .expect("write core lib.rs");
        std::fs::write(root.join("crates/api/src/lib.rs"), "// api lib").expect("write api lib.rs");

        let mut guards = BTreeMap::new();
        guards.insert(
            "worker_agent".to_owned(),
            AgentGuard {
                allowed_dirs: vec!["src".to_owned()],
                ..Default::default()
            },
        );

        // 1. Scoped path with crate prefix "core/hasher.rs" -> "crates/core/src/hasher.rs"
        assert_eq!(
            resolve_scoped_path_str(
                &root,
                "core/hasher.rs",
                None,
                Some(("worker_agent", &guards))
            ),
            "crates/core/src/hasher.rs"
        );

        // 2. Scoped path with "crates/core/hasher.rs" -> "crates/core/src/hasher.rs"
        assert_eq!(
            resolve_scoped_path_str(
                &root,
                "crates/core/hasher.rs",
                None,
                Some(("worker_agent", &guards))
            ),
            "crates/core/src/hasher.rs"
        );

        // 3. Unique file across crates ("hasher.rs") -> "crates/core/src/hasher.rs"
        assert_eq!(
            resolve_scoped_path_str(&root, "hasher.rs", None, Some(("worker_agent", &guards))),
            "crates/core/src/hasher.rs"
        );

        // 4. Ambiguous file ("lib.rs") in multiple crates falls back without guessing
        assert_eq!(
            resolve_scoped_path_str(&root, "lib.rs", None, Some(("worker_agent", &guards))),
            "src/lib.rs"
        );

        // 5. Creating new file in crate: "core/token.rs" -> "crates/core/src/token.rs"
        assert_eq!(
            resolve_scoped_path_str(
                &root,
                "core/token.rs",
                None,
                Some(("worker_agent", &guards))
            ),
            "crates/core/src/token.rs"
        );

        // 6. Creating new file with full crates prefix: "crates/core/token.rs" -> "crates/core/src/token.rs"
        assert_eq!(
            resolve_scoped_path_str(
                &root,
                "crates/core/token.rs",
                None,
                Some(("worker_agent", &guards))
            ),
            "crates/core/src/token.rs"
        );

        drop(std::fs::remove_dir_all(&root));
    }

    #[test]
    fn test_strip_ansi() {
        let raw = "\u{1b}[2m\u{1b}[0m\u{1b}[38;2;84;142;200mcrate\u{1b}[0m::\u{1b}[38;2;84;142;200mserver\u{1b}[0m";
        assert_eq!(strip_ansi(raw), "crate::server");
    }

    #[test]
    fn test_check_manifest_protection() {
        let cargo_toml = Path::new("Cargo.toml");
        let src_file = Path::new("src/main.rs");

        // Normal source files are not protected
        assert!(check_manifest_protection(src_file, Some("reviewer"), None).is_ok());

        // Manifest file without guard: coder is allowed
        assert!(check_manifest_protection(cargo_toml, Some("coder"), None).is_ok());

        // Manifest file with unauthorized agent: blocked
        let guard = AgentGuard {
            allowed_files: vec!["README.md".to_owned()],
            ..Default::default()
        };
        let err = check_manifest_protection(cargo_toml, Some("reviewer"), Some(&guard));
        assert!(err.is_err());
        assert!(err.unwrap_err().contains("Manifest protection violation"));

        // Manifest file with explicit wildcard or explicit allowance: allowed
        let allowed_guard = AgentGuard {
            allowed_files: vec!["Cargo.toml".to_owned()],
            ..Default::default()
        };
        assert!(
            check_manifest_protection(cargo_toml, Some("reviewer"), Some(&allowed_guard)).is_ok()
        );
    }
}
