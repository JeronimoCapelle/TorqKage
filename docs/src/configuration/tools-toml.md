# Tools Configuration (`tools.toml`)

The `tools.toml` configuration file defines declarative Model Context Protocol (MCP) tools exposed to AI agents by the `agy-mcp` server. It acts as a zero-code tool engine: instead of compiling new Rust logic for every CLI utility, static analysis check, or code modifier, tools are declared declaratively in TOML. The engine automatically synthesizes MCP-compliant JSON Schemas, validates incoming arguments against strict regular expressions, enforces path confinement, monitors safety policies, and optionally executes commands inside a Bubblewrap container when `enforce_sandbox = true` is declared.

---

## The 3-Tier Declarative Inheritance Architecture

`tools.toml` employs a three-tier inheritance model designed to eliminate redundancy while preserving fine-grained control:

$$\text{Category Defaults } [\text{categories}.\ast] \longrightarrow \text{Base Tool Templates } [\text{base\_tools}.\ast] \longrightarrow \text{Concrete Tools } [\text{tools}.\ast]$$

```toml
# Tier 1: Category Defaults
[categories.compilation]
category = "compilation"
tracks_circuit_breaker = true
require_gates = ["vcs_conflicts"]
timeout_seconds = 60

# Tier 2: Base Tool Template
[base_tools.cargo_base]
inherit = "categories.compilation"
action = "static_command"
enforce_workspace_bounds = true
output_capture = "both"

# Tier 3: Concrete Tool Definition
[tools.cargo_check]
inherit = "base_tools.cargo_base"
command = ["cargo", "check", "--workspace", "--all-targets"]
description = "Fast syntax, type, and borrow check across all workspace crates"
```

### Precedence & Override Rules

1. **Attribute Precedence:** Concrete tool attributes override base tool template attributes, which in turn override category defaults (`tools.<name>` > `base_tools.<name>` > `categories.<name>`).
2. **Gate Accumulation:** Gate requirements declared across tiers are accumulated additively into a unified requirement set.
3. **Selective Opt-Out (`skip_gates`):** A concrete tool can exempt itself from specific inherited gates using `skip_gates = ["vcs_conflicts"]`, or bypass all inherited gates with `skip_gates = ["*"]` or `["all"]`.

---

## Complete Setting Reference

### 1. Structural & Inheritance Settings

| Setting | Type | Default | Description & Valid Variants |
| :--- | :--- | :--- | :--- |
| `inherit` | String | `None` | Reference to a parent template in `[base_tools.<name>]` or `[categories.<name>]`. The child tool inherits all fields and selectively overrides specified values. |
| `category` | String | `None` | Logical classification grouping tools together (e.g., `"compilation"`, `"filesystem"`, `"diagnostics"`, `"vcs"`, `"release"`, `"documentation"`, `"systems"`, `"orchestration"`). Inherits category defaults when set. |
| `description` | String | `""` | LLM-facing documentation string explaining the purpose, behavior, preconditions, and return value of the tool. Synthesized directly into the MCP JSON Schema. |

---

### 2. Execution Actions (`action`)

The `action` field selects the underlying execution engine powering the tool. There are 11 distinct action variants:

| Action Variant | Description & Capabilities | Required Parameters / Arguments |
| :--- | :--- | :--- |
| `file_read` | Bounded file reading with automatic line/byte truncation, utf-8 validation, and truncation notices. Bounded to workspace. | `path` (String), `start_line` (optional Int), `end_line` (optional Int) |
| `file_write` | Atomic file writer with parent directory auto-creation, strict workspace boundary enforcement, and dangerous path protection. | `path` (String), `content` (String), `overwrite` (optional Bool) |
| `mpatch_edit` | High-precision fuzzy search-and-replace block patcher powered by the mpatch engine. Verifies surrounding context before replacement. | `path` (String), `target` (String), `replacement` (String) |
| `file_search` | Scoped grep/ripgrep pattern search across repository source files or configured project documentation paths. | `query` (String), `paths` (optional Array) |
| `static_command` | Executes a fixed command-line invocation without dynamic user-supplied arguments. Safe against shell injection. | `command` (Array of Strings) |
| `parameterized_command` | Dynamic CLI tool execution where user arguments are mapped to flags and validated against strict regular expressions. | `command` (Array of Strings), `args` (Array of `ToolArg`) |
| `cli_manual` | Formats and renders man-page style help text for CLI utilities and engine tools. | Built-in |
| `project_state` | Synthesizes active VCS revision, dirty file status, Jujutsu working copy commit, and quality gate health into an executive summary. | Built-in |
| `workspace_ensure` | Provisions, switches to, or validates an isolated Jujutsu sparse workspace for a subagent (`.agy/workspaces/<agent>/`). | `agent` (String) |
| `workspace_inspect` | Inspects working copy diffs and file modifications between subagent sparse workspaces and the main working tree. | `agent` (String) |
| `workspace_merge` | Absorbs or merges changes from an isolated subagent workspace back into the active working copy. | `agent` (String) |

---

### 3. Argument Specification (`args`)

Parameterized commands define their input arguments in the `args` array:

```toml
[tools.run_single_test]
action = "parameterized_command"
command = ["cargo", "test"]
description = "Runs a specific unit or integration test by name"
enforce_sandbox = true
args = [
    { name = "test_name", required = true, regex = "^[a-zA-Z0-9_:]+$", description = "The exact name of the test function or filter" },
    { name = "exact", required = false, flag = "--exact", type = "boolean", description = "Match test name exactly" },
    { name = "target", required = false, enum = ["lib", "bins", "examples"], flag = "--", description = "Target type to execute" }
]
```

#### Argument Fields

| Field | Type | Default | Description |
| :--- | :--- | :--- | :--- |
| `name` | String | *Required* | Parameter property name in the generated MCP JSON Schema. |
| `type` | String enum | `"string"` | Argument data type: `"string"`, `"integer"`, `"boolean"`, `"path"`, or `"enum"`. |
| `description` | String | `""` | Detailed explanation of the argument provided to the LLM. |
| `required` | Boolean | `false` | When `true`, callers must supply this argument; calls omitting it are rejected before execution. |
| `regex` | String | `None` | Strict regular expression pattern that the argument value must match. Defends against shell injection and malicious input. |
| `enum` | Array of strings | `[]` | Allowed discrete values when `type = "enum"`. Rejects any argument value not matching one of the variants. |
| `conflicts_with` | Array of strings | `[]` | Names of other arguments that cannot be supplied simultaneously (enforcing mutually exclusive flags). |
| `flag` | String | `None` | Optional CLI flag prepended to the argument when constructing the command line (e.g., `"-p"`, `"--bin"`, `"-C"`, `"--"`). |
| `default` | String | `None` | Default value injected if the caller omits the argument. |

---

### 4. Safety & Policy Flags

| Setting | Type | Default | Description & Valid Variants |
| :--- | :--- | :--- | :--- |
| `mutating` | Boolean | `false` | Marks the tool as state-altering (modifying files, branches, or workspace state). Monitored by `manifest_guard` and `circuit_breaker` in `agy-hooks`. |
| `tracks_circuit_breaker` | Boolean | `false` | When `true`, non-zero exit codes from this tool add a failure strike to `.agy/state/circuit_breaker.json`. Once strikes reach `max_strikes`, mutating tools are blocked. |
| `require_permission_gate` | Boolean | `false` | When `true`, pauses execution and displays an interactive confirmation prompt (`[y/n]`) in the control deck or TUI before the tool executes. |

---

### 5. Sandboxing & Filesystem Boundaries

| Setting | Type | Default | Description & Valid Variants |
| :--- | :--- | :--- | :--- |
| `enforce_sandbox` | Boolean | `false` | When `true`, the process is jailed inside a Bubblewrap container with a read-only root mount and isolated temporary directories.<br>Alias: `sandbox`. |
| `enforce_workspace_bounds` | Boolean | `true` | Prevents path traversal vulnerabilities (`../`) and rejects any file paths resolving outside the workspace root. |
| `ban_dangerous_files` | Boolean | `true` | Prevents the tool from reading, writing, or inspecting sensitive files (SSH keys, credentials, `.git/`, `.jj/`, shell rc files). |
| `allowed_directories` | Array of strings | `[]` | Explicit directory whitelist. If non-empty, file operations are restricted strictly to these relative directories. |
| `enforce_dir` | String | `None` | Mandatory working directory for process execution. |
| `enforce_file_target` | String | `None` | Mandatory destination file constraint for write/modify tools. |
| `sanitize_input` | String enum | `None` | Sanitization profile applied to command arguments before execution (`"strict_path"`, `"alphanumeric"`). |

---

### 6. Output Capture & Truncation

| Setting | Type | Default | Description & Valid Variants |
| :--- | :--- | :--- | :--- |
| `output_capture` | String enum | `"stdout"` | Process streams captured and returned to the LLM:<br>• `stdout`: Captures standard output only.<br>• `stderr`: Captures standard error only.<br>• `both`: Interleaves combined stdout and stderr.<br>• `exit_code_only`: Returns `"Success (exit code 0)"` without token output.<br>• `json_spans`: Filters compiler JSON diagnostic streams to actionable error spans. |
| `truncate_lines` | Integer (usize) | `None` | Maximum number of output lines returned before truncating. |
| `truncate_bytes` | Integer (usize) | `None` | Maximum byte length of output returned before truncating. |
| `emit_truncation_notice` | Boolean | `true` | Appends `"[Output truncated: ... lines omitted]"` when output limits are reached. |
| `timeout_seconds` | Integer (u64) | `15` | Maximum execution duration before terminating the child process with `SIGKILL`. |

---

### 7. Preconditions, Gates & Fallbacks

| Setting | Type | Default | Description & Valid Variants |
| :--- | :--- | :--- | :--- |
| `require_gates` | Array of strings | `[]` | Array of quality gate identifiers (native gates like `"vcs_conflicts"`, or custom gates from `project.toml`) that must pass before tool execution proceeds. |
| `skip_gates` | Array of strings | `[]` | Array of gate names to exempt from category inheritance. Use `["*"]` or `["all"]` to bypass all inherited gates. |
| `empty_fallback` | String | `None` | Replacement text returned to the LLM if the command completes with code 0 but produces zero output bytes (e.g., `"Workspace check passed with 0 warnings."`). |
| `error_fallback` | String | `None` | Custom diagnostic explanation returned when the process terminates with a non-zero exit code. |
| `success_exit_codes` | Array of integers | `[0]` | Array of process exit codes considered successful (e.g., `[0, 1]` for grep utilities that return 1 on no matches). |

---

## Declarative Tool Examples

### 1. Bounded File Modifier (`mpatch_edit`)

```toml
[tools.modify_code]
category = "edit"
action = "mpatch_edit"
description = "High-precision search-and-replace code modification using the mpatch engine"
mutating = true
enforce_workspace_bounds = true
ban_dangerous_files = true
require_permission_gate = false
```

### 2. Isolated Hardware Disassembly Tool (`parameterized_command`)

```toml
[tools.disassemble_symbol]
category = "systems"
action = "parameterized_command"
command = ["llvm-objdump", "-d", "--no-show-raw-insn"]
description = "Disassembles machine code for a specific symbol in the target binary"
enforce_sandbox = true
output_capture = "stdout"
truncate_lines = 100
timeout_seconds = 10

[[tools.disassemble_symbol.args]]
name = "binary_path"
description = "Path to compiled ELF or binary under target/"
required = true
regex = "^target/[a-zA-Z0-9_/.-]+$"

[[tools.disassemble_symbol.args]]
name = "symbol_name"
description = "Function symbol to disassemble"
required = true
regex = "^[a-zA-Z0-9_:]+$"
flag = "--disassemble-symbols"
```

### 3. Compiler Check with Concise Diagnostics (`static_command`)

```toml
[tools.cargo_check]
inherit = "categories.compilation"
action = "static_command"
command = ["cargo", "check", "--workspace", "--all-targets", "--message-format=short", "--color=never"]
description = "Runs cargo check across all crates and returns actionable compiler diagnostics"
tracks_circuit_breaker = true
output_capture = "both"
empty_fallback = "Verification passed: 0 errors, 0 warnings."
```

---

## Discovery & Precedence

The `agy-mcp` daemon and `agy-hooks` engine discover `tools.toml` across the following locations:

1. `<workspace>/.agy/tools.toml` *(Workspace-local custom override)*
2. `<workspace>/tools.toml` *(Per-repository project overrides)*
3. `<workspace>/plugins/agy-suite/tools.toml` *(In-tree plugin bundle default)*
4. `~/.gemini/config/plugins/agy-suite/tools.toml` *(Global user configuration)*
