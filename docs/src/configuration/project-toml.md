# Project Configuration (`project.toml`)

The `project.toml` file defines repository identity, compiler quality gates, workspace layout, Bubblewrap sandbox bind-mounts, environment variables, security guardrails, and documentation locations. It completely decouples language-specific build tools, compilers, test runners, and build caches from the `TorqKage` core engine.

When `project.toml` is present in the repository, `tkg`, `tkg-mcp`, and `tkg-hooks` read its declarations to validate code changes, manage build caches, and protect sensitive repository paths.

---

## Complete Schema & Annotated Example

```toml
# ==============================================================================
# 1. PROJECT METADATA
# ==============================================================================
[project]
name = "hyperion-engine"
language = "rust"
version = "0.4.0"

# ==============================================================================
# 2. WORKSPACE LAYOUT & BUILD CACHES
# ==============================================================================
[workspace]
containers = ["crates", "packages"]
ignored_paths = ["target", ".zig-cache", "build"]

# ==============================================================================
# 3. COMPILER & QUALITY GATES
# ==============================================================================
[gates.compilation]
command = ["cargo", "check", "--workspace", "--all-targets"]
timeout_seconds = 30
failure_message = "Cargo compilation check failed. Fix type and borrow errors before proceeding."
error_regex = "^error(\\[E[0-9]+\\])?:"
explain_command = ["rustc", "--explain"]

[gates.tests]
command = ["cargo", "nextest", "run", "--workspace"]
timeout_seconds = 60
failure_message = "Test suite regressions detected. Verify unit test assertions."

[gates.linter]
command = ["cargo", "clippy", "--workspace", "--all-targets", "--", "-D", "warnings"]
timeout_seconds = 45
failure_message = "Clippy linter found code style or correctness violations."

# ==============================================================================
# 4. SANDBOX JAIL MOUNTS
# ==============================================================================
[sandbox]
bind_mounts = ["target"]

# ==============================================================================
# 5. ENVIRONMENT VARIABLES
# ==============================================================================
[environment]
RUST_BACKTRACE = "1"
CARGO_TERM_COLOR = "never"
RUSTFLAGS = "-D warnings"

# ==============================================================================
# 6. SECURITY & SENSITIVE FILE PROTECTION
# ==============================================================================
[security]
block_generic_credentials = true
block_vcs_internals = true
block_ssh_keys = true
block_shell_profiles = true
block_sensitive_extensions = true
extra_banned_patterns = ["secrets.yaml", "internal_keys/*"]
allowed_patterns = ["tests/fixtures/mock_secret.key"]

# ==============================================================================
# 7. DOCUMENTATION & SPECIFICATIONS
# ==============================================================================
[docs]
paths = ["docs", "specs", "ARCHITECTURE.md"]
description = "Consult ARCHITECTURE.md for memory layout invariants before modifying core code"
tool_name = "query_project_spec"

# ==============================================================================
# 8. GLOBAL POLICIES & SYNTAX RULES
# ==============================================================================
[policies.syntax]
line_comments = ["//"]
test_attributes = ["#[test]", "#[cfg(test)]", "#[tokio::test]"]
test_prefixes = ["test_"]

[[path_policies]]
path = "crates/kernel"
filter = "comments_only"
```

---

## Section-by-Section Reference

### 1. Project Metadata (`[project]`)

| Field | Type | Default | Description & Valid Variants |
| :--- | :--- | :--- | :--- |
| `name` | String | `None` | The human-readable or package name of the project. |
| `language` | String | Auto-inferred | Primary programming language or ecosystem. Valid variants:<br>• `"rust"` / `"rs"`<br>• `"zig"`<br>• `"go"` / `"golang"`<br>• `"python"` / `"py"`<br>• `"c"`<br>• `"cpp"` / `"c++"`<br>• `"typescript"` / `"ts"` / `"javascript"`<br>• `"shell"` / `"bash"`<br>• `"lua"` / `"sql"` |
| `version` | String | `None` | Semantic version string of the active project. |

---

### 2. Workspace Layout (`[workspace]`)

| Field | Type | Default | Description |
| :--- | :--- | :--- | :--- |
| `containers` | Array of strings | Auto-inferred | Subdirectory names holding member packages in a monorepo (e.g., `["crates", "packages", "apps", "libs"]`). Auto-detects `crates` or `packages` if present on disk. |
| `ignored_paths` | Array of strings | Auto-inferred | Build output directories, temporary artifacts, and cache paths excluded from VCS status checks, diff previews, and search indexing (e.g., `["target", ".zig-cache", "build"]`). |

---

### 3. Compiler & Quality Gates (`[gates.<id>]`)

Gates are declarative validation commands executed by `tkg-hooks` (during subagent admission or tool preconditions) and `tkg` (flight deck health checks).

| Field | Type | Default | Description & Valid Variants |
| :--- | :--- | :--- | :--- |
| `command` | Array of strings | *Required* | Command tokens executed to validate the gate (e.g., `["cargo", "check", "--workspace"]`). |
| `timeout_seconds` | Integer (u64) | `30` (compilation), `45` (test) | Maximum execution duration before timing out and failing the gate check. |
| `failure_message` | String | `None` | Custom error message returned to the agent when this gate fails, providing guidance on how to fix the issue. |
| `error_regex` | String | `None` | Regular expression pattern used to extract the root compiler or test error diagnostic lines from process output. |
| `explain_command` | Array of strings | `None` | Optional CLI command executed on gate failure with the extracted error code appended (e.g., `["rustc", "--explain"]`). |

---

### 4. Sandbox Configuration (`[sandbox]`)

| Field | Type | Default | Description |
| :--- | :--- | :--- | :--- |
| `bind_mounts` | Array of strings | Auto-inferred | Relative directories that must be bind-mounted read-write inside the Bubblewrap sandbox jail (e.g., `["target", ".zig-cache", "build"]`). Allows compilers to reuse incremental build caches without compromising filesystem confinement. |

---

### 5. Environment Variables (`[environment]`)

The `[environment]` table contains arbitrary key-value string pairs injected into child process environments during tool execution and quality gate validation:

```toml
[environment]
RUST_BACKTRACE = "1"
CARGO_TERM_COLOR = "never"
CC = "clang"
```

---

### 6. Security & Sensitive File Protection (`[security]`)

`TorqKage` includes an active security sentry that protects sensitive keys, credentials, and configuration files from being read, modified, or exfiltrated by agents.

| Field | Type | Default | Description & Protected Targets |
| :--- | :--- | :--- | :--- |
| `block_generic_credentials` | Boolean | `true` | Blocks files whose names contain credential tokens: `password`, `credential`, `secret`, `api_key`, `token`. |
| `block_vcs_internals` | Boolean | `true` | Blocks version control repository metadata and internal databases: `.git`, `.jj`, `.hg`, `.svn`. |
| `block_ssh_keys` | Boolean | `true` | Blocks private keys and keychains: `id_rsa`, `id_ed25519`, `id_ecdsa`, `known_hosts`, `authorized_keys`. |
| `block_shell_profiles` | Boolean | `true` | Blocks user and environment shell initialization scripts: `.bashrc`, `.bash_profile`, `.zshrc`, `.profile`. |
| `block_sensitive_extensions` | Boolean | `true` | Blocks sensitive key and certificate extensions: `.key`, `.pem`, `.pfx`, `.p12`, `.crt`, `.der`. |
| `disabled_categories` | Array of strings | `[]` | Explicit array of security categories to unblock if needed for specialized workflows (e.g., `["generic_credentials"]`). |
| `extra_banned_patterns` | Array of strings | `[]` | Additional project-specific sensitive filenames or glob patterns blocked from agent access. Alias: `extra_banned_files`. |
| `allowed_patterns` | Array of strings | `[]` | Whitelist patterns permitted to bypass security category blocks (e.g., mock test certificates or test fixtures). |

---

### 7. Documentation & Specifications (`[docs]`)

| Field | Type | Default | Description |
| :--- | :--- | :--- | :--- |
| `paths` | Array of strings | Auto-detected | Paths to specification files, architecture guides, and documentation directories (e.g., `["docs", "specs", "ARCHITECTURE.md"]`). Aliases: `dirs`, `docs_paths`. |
| `description` | String | `None` | Instructions injected into agent prompts on SessionStart explaining how and when to consult documentation. |
| `tool_name` | String | `None` | Name of the recommended tool exposed to agents for searching specifications (e.g., `"query_project_spec"`). |

---

### 8. Policies & Syntax Overrides (`[policies]` / `[[path_policies]]`)

Configures language-agnostic parsing rules and project-wide content filter policies:

```toml
[policies.syntax]
line_comments = ["#"]
block_comments = [{ start = "'''", end = "'''" }]
test_attributes = ["@pytest.mark"]
test_prefixes = ["test_"]

[[path_policies]]
path = "src/legacy"
filter = "comments_only"
```

| Field | Type | Default | Description |
| :--- | :--- | :--- | :--- |
| `line_comments` | Array of strings | Auto-inferred | Comment prefixes for line comments. |
| `block_comments` | Array of objects | Auto-inferred | Block comment delimiter pairs (`start` and `end`). |
| `test_attributes` | Array of strings | Auto-inferred | Annotations or decorators marking test items. |
| `test_prefixes` | Array of strings | Auto-inferred | Function name prefixes denoting tests. |
| `path_policies` | Array of objects | `[]` | Project-wide path policies applied across all agents. |

---

### 9. Execution Harness & Agent System Runner (`[runner]` / `[harness]`)

`TorqKage` is harness-agnostic. It works across popular agent execution harnesses (Claude Code, OpenCode, Cursor, Google Antigravity, and custom CLI runners), with ongoing expansion to support additional common harnesses:

```toml
[runner]
harness = "opencode"             # "agy" (default), "opencode", "claude", or "custom"
command = ["opencode"]           # Optional command override or template
agent_flag = "--agent"           # Flag used to dispatch to an agent persona
sandbox_flag = "--sandbox"       # Optional sandbox flag passed to harness
resume_flag = "--session"        # Flag used to resume prior conversation sessions
extra_args = ["--verbose"]       # Additional CLI arguments passed on launch
agent_dirs = ["agents", ".opencode/agents"] # Optional explicit agent discovery search paths
```

The harness is chosen by `[runner] harness` in `project.toml` and, at install time, by `./install.sh --harness <name>` (which selects the global bundle location and records the choice in the installed `project.toml`). Unknown names are treated as custom harnesses and use convention-based paths (`.<name>/agents`, `~/.<name>/plugins/torqkage`) with the harness name as the default binary. All configuration discovery (`tools.toml`, `hooks.toml`, plugin directories) is centralized in `tkg_common::harness`.

| Field | Type | Default | Description |
| :--- | :--- | :--- | :--- |
| `harness` | String | `"agy"` | Execution platform: `"agy"`, `"opencode"`, `"claude"`, or `"custom"`. Determines the default binary name and session discovery backend. |
| `command` | Array of strings or String | Auto-inferred | Optional explicit command. If array elements contain `{agent}` or `{session}`, they are replaced with the target agent and conversation ID. |
| `agent_flag` | String | Auto-inferred (`"--agent"`) | CLI flag used to specify the agent persona. |
| `sandbox_flag` | String | `"--sandbox"` (for `agy`), `None` (for others) | Optional CLI flag to pass when Bubblewrap sandboxing is toggled. |
| `resume_flag` | String | `"--conversation"` (for `agy`), `"--session"` (for `opencode`), `"--resume"` (for others) | CLI flag passed when resuming an existing session. |
| `extra_args` | Array of strings | `[]` | Additional flags or arguments appended to the harness command. |
| `agent_dirs` | Array of strings | `[]` | Explicit directories to discover agent markdown definitions before searching standard workspace paths. |

---

## Ecosystem Auto-Detection & Inference

When `project.toml` is absent or specific fields are omitted, `TorqKage` automatically infers sane configuration defaults by inspecting manifest indicators on disk:

| Ecosystem Indicator | Inferred `language` | Inferred Containers | Inferred Ignored Paths | Inferred Sandbox Mounts |
| :--- | :--- | :--- | :--- | :--- |
| `Cargo.toml` | `rust` | `crates`, `packages` | `target` | `target` |
| `build.zig` | `zig` | *None* | `.zig-cache`, `zig-out` | `.zig-cache`, `zig-out` |
| `go.mod` | `go` | *None* | `.go-cache` | `.go-cache` |
| `pyproject.toml` | `python` | *None* | `.pytest_cache`, `.mypy_cache`, `__pycache__` | `.pytest_cache`, `.mypy_cache` |
| `package.json` | `typescript` | `packages`, `apps` | `node_modules`, `dist` | `node_modules`, `dist` |
| `meson.build` | `c` | *None* | `build` | `build` |
| `CMakeLists.txt` | `cpp` | *None* | `build` | `build` |

---

## Ecosystem Templates

Ready-to-use production configuration templates are located in `docs/templates/`:

- [`rust-project.toml`](../templates/rust-project.toml): Cargo compilation, test gates, Clippy linter.
- [`typescript-project.toml`](../templates/typescript-project.toml): tsc type checks, Vitest runner, ESLint, Prettier.
- [`python-project.toml`](../templates/python-project.toml): mypy type checks, Pytest, Ruff linter/formatter.
- [`zig-project.toml`](../templates/zig-project.toml): zig build check, zig test, zig fmt.
- [`go-project.toml`](../templates/go-project.toml): go build, go test, golangci-lint, gofmt.
- [`c-cpp-project.toml`](../templates/c-cpp-project.toml): CMake build, CTest, Clang-Tidy, Clang-Format.

---

## Discovery & Precedence

`project.toml` is resolved in the following priority order:

1. `<workspace>/.tkg/project.toml` *(Workspace-local override)*
2. `<workspace>/project.toml` *(Repository root configuration)*
3. `<workspace>/plugins/torqkage/project.toml` *(In-tree plugin default)*
4. `~/.gemini/config/plugins/torqkage/project.toml` *(Global installed plugin fallback)*
5. Dynamic synthesis via ecosystem auto-detection.
