# Hooks Configuration (`hooks.toml`)

The `hooks.toml` configuration file defines the lifecycle execution pipeline, guard sentries, and context injection rules for `tkg-hooks`. The `tkg-hooks` engine evaluates native high-assurance Rust safety primitives in-memory (<1ms latency) alongside declarative custom shell assertions, completely decoupling safety policies from compiled code.

---

## Architectural Role & Lifecycle Events

`hooks.toml` governs how `tkg-hooks` responds to events emitted by the Antigravity host environment:

| Lifecycle Event | `tkg-hooks` Command | Description & Purpose |
| :--- | :--- | :--- |
| `PreToolUse` | `tkg-hooks pre-tool-use` | Evaluated before **any** tool call executes. Executes the pipeline to return `allow`, `deny`, or `ask`. |
| `PreInvocation` | `tkg-hooks detect-wip` | Evaluated before each agent turn. Injects turn-by-turn diff notices and modified file summaries. |
| `SessionStart` | `tkg-hooks inject-wake` | Evaluated on Turn 1. Injects session handoff notes, VCS state, documentation paths, and automated wake queries. |
| `Stop` | `tkg-hooks record-handoff` | Evaluated on session termination. Captures structured JSON and Markdown summaries into `.tkg/state/`. |

### Decision Outcomes

When evaluating tool calls, `tkg-hooks` produces one of three deterministic outcomes:

- `allow`: The operation is safe; execution proceeds immediately without disruption.
- `deny`: The operation violates security policies, manifest boundaries, or circuit breaker thresholds; execution is aborted, and an explanatory rejection notice is returned to the agent prompt.
- `ask`: The operation involves supervised changes; execution pauses, and an interactive confirmation prompt (`[y/n]`) is displayed to the human operator.

---

## Complete Schema & Annotated Example

```toml
# ==============================================================================
# 1. PRE-TOOL-USE PIPELINE & GUARD ORDER
# ==============================================================================
[pre_tool_use]
pipeline = [
    "circuit_breaker",
    "manifest_guard",
    "path_policies",
    "subagent_admission",
    "vcs_conflicts",
    "tool_policy",
]

# ==============================================================================
# 2. CIRCUIT BREAKER CONFIGURATION
# ==============================================================================
[circuit_breaker]
enabled = true
max_strikes = 2

# ==============================================================================
# 3. MANIFEST MUTATION GUARD
# ==============================================================================
[manifest_guard]
enabled = true
protected_patterns = [
    "Cargo.toml",
    "Cargo.lock",
    "build.rs",
    "CMakeLists.txt",
    "meson.build",
    "package.json",
    "build.zig",
    "go.mod",
    "Makefile",
]
protected_tools = []

# ==============================================================================
# 4. WORK-IN-PROGRESS (WIP) CHANGE DETECTOR
# ==============================================================================
[wip]
enabled = true
max_preview_files = 4

# ==============================================================================
# 5. SESSION START CONTEXT & AUTOMATED WAKE QUERIES
# ==============================================================================
[session_start]
enabled = true
inject_wake_state = true
inject_documentation = true

[[session_start.injections]]
name = "finisher_diff_summary"
matcher = "finisher"
command = ["jj", "diff", "--summary"]
max_lines = 40
timeout_seconds = 5
prefix = "### [AUTOMATED WAKE CONTEXT]: Active Workspace Diff Summary\n"
fallback = "Working copy has no uncommitted modifications."

[[session_start.injections]]
name = "copilot_vcs_status"
matcher = "copilot"
command = ["jj", "status", "--no-pager"]
max_lines = 15
timeout_seconds = 5
prefix = "### [AUTOMATED WAKE CONTEXT]: Active Jujutsu VCS Status\n"
fallback = "VCS status clean."

# ==============================================================================
# 6. SESSION STOP HANDOFF RECORDING
# ==============================================================================
[stop]
enabled = true
record_handoff = true
max_modified_files = 10

# ==============================================================================
# 7. DECLARATIVE CUSTOM CHECKS
# ==============================================================================
[[pre_tool_use.custom_checks]]
name = "disk_space_sentry"
matcher = "modify_code|write_file"
command = ["df", "--output=pcent", "."]
fail_if_output_contains = "100%"
decision = "deny"
reason = "Disk space critical: filesystem write operations blocked."
```

---

## Section-by-Section Reference

### 1. Pre-Tool-Use Pipeline (`[pre_tool_use]`)

The `pipeline` array defines the sequential chain of native Rust guard primitives evaluated on every tool invocation. Evaluation stops immediately on the first non-`allow` decision (first `deny` or `ask`).

| Native Primitive | Description & Enforcement Role |
| :--- | :--- |
| `"circuit_breaker"` | Checks consecutive compiler failure strikes in `.tkg/state/circuit_breaker.json`. Blocks mutating tools if strikes reach `max_strikes`. Non-mutating tools remain permitted. |
| `"manifest_guard"` | Defends against build-time remote code execution (RCE) by blocking agent mutation of build manifests (`Cargo.toml`, `build.rs`, `package.json`, `build.zig`) without operator confirmation. |
| `"path_policies"` | Enforces directory whitelists (`allowed_dirs`, `disallowed_dirs`), file boundaries, and fine-grained content filters (`comments_only`, `tests_only`, `delimited`) declared in agent manifests. |
| `"subagent_admission"` | Intercepts `invoke_subagent`, parses the target agent's `pre_launch_checks`, and runs matching quality gates from `project.toml`. Denies spawn if gates fail. |
| `"vcs_conflicts"` | Scans the workspace for unresolved Git (`<<<<<<<`) or Jujutsu merge conflict markers before running compilation or test tools. |
| `"tool_policy"` | Checks `mutating` and `require_permission_gate` flags in `tools.toml`. Emits `ask` to prompt the operator for confirmation if supervised execution is mandated. |

---

### 2. Declarative Custom Checks (`[[pre_tool_use.custom_checks]]`)

Developers can add project-specific checks without recompiling Rust binaries:

| Field | Type | Default | Description & Valid Variants |
| :--- | :--- | :--- | :--- |
| `name` | String | *Required* | Descriptive identifier for the custom check. |
| `matcher` | String (regex) | *Required* | Regular expression matching tool names that trigger this check (e.g., `"modify_code|write_file"` or `".*"`). |
| `command` | Array of strings | *Required* | Command tokens executed on the host system. |
| `fail_if_output_contains` | String | `None` | Substring within combined stdout/stderr that triggers failure. If omitted, failure is triggered by non-zero exit code. |
| `decision` | String enum | `"deny"` | Hook outcome on failure:<br>• `"deny"`: Blocks the tool call and returns `reason` to the agent.<br>• `"ask"`: Prompts the operator `[y/n]` with `reason`.<br>• `"allow"`: Informational check only. |
| `reason` | String | *Required* | Explanatory message provided to the agent or operator. |

---

### 3. Circuit Breaker (`[circuit_breaker]`)

The circuit breaker prevents autonomous agents from entering infinite compile-fail-retry loops:

| Field | Type | Default | Description |
| :--- | :--- | :--- | :--- |
| `enabled` | Boolean | `true` | Enables or disables circuit breaker enforcement. |
| `max_strikes` | Integer (usize) | `2` | Number of consecutive compiler failures before tripping. Set to `0` to disable strike counting.<br>When tripped:<br>• Non-mutating diagnostic tools (`read_project_file`, `query_project_spec`) remain **allowed**.<br>• Mutating tools (`modify_code`, `write_file`) are **denied** until the circuit is reset.<br>Resetting: Press `c` (or `Shift+K`) in `tkg` or achieve a clean compilation run. |

---

### 4. Manifest Mutation Guard (`[manifest_guard]`)

Protects build scripts and configuration manifests from unauthorized agent tampering:

| Field | Type | Default | Description |
| :--- | :--- | :--- | :--- |
| `enabled` | Boolean | `true` | Enables or disables manifest protection. |
| `protected_patterns` | Array of strings | Standard build manifests | Filenames and glob patterns protected against mutation (e.g., `["Cargo.toml", "Cargo.lock", "build.rs", "CMakeLists.txt", "package.json", "build.zig", "go.mod"]`). |
| `protected_tools` | Array of strings | `[]` | Explicit tool names to intercept. If empty (`[]`), `tkg-hooks` dynamically inspects `tools.toml` and protects all tools flagged with `mutating = true`, plus built-in host tools (`write_file`, `edit_file`). Automatically detects file path parameters across all naming conventions (`path`, `Path`, `destination`, `file`). |

---

### 5. Work-in-Progress (WIP) Detector (`[wip]`)

Evaluated on `PreInvocation` before each agent turn:

| Field | Type | Default | Description |
| :--- | :--- | :--- | :--- |
| `enabled` | Boolean | `true` | Enables turn-by-turn diff notices in the agent prompt. |
| `max_preview_files` | Integer (usize) | `4` | Maximum number of modified files displayed in the WIP prompt preview. |

---

### 6. Session Start & Automated Wake Queries (`[session_start]`)

Evaluated on `SessionStart` (Turn 1):

| Field | Type | Default | Description |
| :--- | :--- | :--- | :--- |
| `enabled` | Boolean | `true` | Enables Turn 1 context injection. |
| `inject_wake_state` | Boolean | `true` | Injects the previous session handoff summary, active Jujutsu change ID, branch name, and dirty working copy status. |
| `inject_documentation` | Boolean | `true` | Injects configured documentation paths and recommended search tool from `project.toml`. |

#### Automated Wake Injections (`[[session_start.injections]]`)

Automated wake queries execute host CLI queries on Turn 1 and inject structured data directly into the agent prompt, eliminating round-trip tool calls and saving token budget:

| Field | Type | Default | Description |
| :--- | :--- | :--- | :--- |
| `name` | String | *Required* | Unique injection identifier. |
| `matcher` | String | `"*"` | Persona matcher string or regex. Matches against the active persona name (e.g., `"finisher"`, `"copilot"`, `"*"` for all personas). |
| `command` | Array of strings | *Required* | Command tokens executed on the host (e.g., `["jj", "diff", "--summary"]`). |
| `max_lines` | Integer (usize) | `40` | Maximum lines captured before truncation. |
| `timeout_seconds` | Integer (u64) | `5` | Command execution timeout before aborting. |
| `prefix` | String | `""` | Markdown header prepended to the command output in the prompt. |
| `fallback` | String | `None` | Fallback text displayed if the command returns empty output (e.g., `"Working copy clean."`). |

---

### 7. Session Stop Handoff (`[stop]`)

Evaluated on `Stop` when an agent finishes or exits:

| Field | Type | Default | Description |
| :--- | :--- | :--- | :--- |
| `enabled` | Boolean | `true` | Enables session handoff capture. |
| `record_handoff` | Boolean | `true` | Writes structured handoff snapshots to `.tkg/state/handoff.json` and human-readable summaries to `.tkg/state/last_handoff.md`. |
| `max_modified_files` | Integer (usize) | `10` | Maximum number of modified files recorded in the handoff ledger. |

---

## Discovery & Precedence

`hooks.toml` is resolved in the following priority order:

1. `<workspace>/.tkg/hooks.toml` *(Workspace-local custom override)*
2. `<workspace>/hooks.toml` *(Repository root configuration)*
3. `<workspace>/plugins/torqkage/hooks.toml` *(In-tree plugin bundle default)*
4. `~/.gemini/config/plugins/torqkage/hooks.toml` *(Installed plugin global fallback)*
