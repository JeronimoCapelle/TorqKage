# Agent Manifests (`AGENT.md`)

Agent manifests define persona identity, reasoning effort, permission boundaries, sandboxing policies, tool whitelists, admission prerequisites, and behavioral directives. In `agy-suite`, agent manifests are authored as Markdown documents with YAML frontmatter bounded by triple-dash (`---`) fences.

Manifest files reside in `plugins/agy-suite/agents/<name>.md`, `<workspace>/agents/<name>.md`, or custom locations, where `<name>` corresponds to the persona identifier (such as `coder`, `test-author`, `doc-author`, `finisher`, `planner`, or `copilot`).

---

## Complete Schema & Variant Specification

An agent manifest consists of two distinct regions:
1. **YAML Frontmatter (`---` to `---`)**: Structured configuration declaring execution policies, security gates, and tool allowances.
2. **Directives Body (Markdown text after second `---`)**: Natural language instructions, invariant rules, and domain-specific mission statements passed directly to the model as its system prompt.

### Annotated Example Manifest

```yaml
---
name: test-author
description: "Design Verification (DV) & Formal Test Engineer spanning differential testing, Kani proofs, Loom, Miri, and mutation coverage"
effort: medium
permissionMode: acceptEdits
commandExecutionPolicy: off
mainAgent: true
subagent: true
inheritMcp: false

allowed_subagents:
  - mini
  - spec-checker

pre_launch_checks:
  - vcs_conflicts
  - compilation

allowed_dirs:
  - tests
  - benches
  - src

disallowed_dirs:
  - docs

allowed_files: []
disallowed_files:
  - ARCHITECTURE.md

path_policies:
  - path: src
    filter: tests_only
    test_attributes:
      - "#[test]"
      - "#[cfg(test)]"
    test_prefixes:
      - "test_"

tools:
  - search_web
  - ask_question

mcpServers:
  - name: agy-tools
    command: agy-mcp
    args: ["--agent", "test-author"]

mcp_tools:
  - read_project_file
  - list_directory
  - inspect_directory_tree
  - find_project_files
  - inspect_file_meta
  - search_codebase
  - query_project_spec
  - write_file
  - modify_code
  - inspect_workspace_diff
---

# Directives
- **Single-Minded Mission (TDD & Verification):**
  - Author rigorous, adversarial test suites verifying systems code under test.
  - You operate strictly under the `tests_only` filter when touching `src/`.
```

---

## Frontmatter Fields & Variants

### 1. Identity & Persona

| Field | Type | Default | Description & Valid Variants |
| :--- | :--- | :--- | :--- |
| `name` | String | *Required* | Canonical persona identifier matching the manifest basename without extension (e.g., `coder`, `planner`, `finisher`). Must contain only lowercase alphanumeric characters, hyphens, and underscores. |
| `description` | String | `""` | Detailed description of the persona's architectural role, capabilities, and when it should be invoked. Displayed in subagent routing catalogs and control deck UI. |
| `mainAgent` | Boolean | `false` | When `true`, this persona can be launched as the primary interactive conversational agent directly by the operator. |
| `subagent` | Boolean | `false` | When `true`, this persona is registered as an autonomous subagent callable via `invoke_subagent`. |

### 2. Reasoning Effort & Execution Policies

| Field | Type | Default | Description & Valid Variants |
| :--- | :--- | :--- | :--- |
| `effort` | String enum | `"medium"` | Controls reasoning effort and chain-of-thought token allocation for thinking models. Valid variants:<br>• `low`: Minimal thinking for rapid, deterministic tasks.<br>• `medium`: Balanced reasoning for typical development (default).<br>• `high`: Deep reasoning for architectural planning and refactoring.<br>• `xhigh`: Extended reasoning for complex mathematical or formal logic proofs.<br>• `max`: Maximum allowable reasoning window. |
| `permissionMode` | String enum | `"acceptEdits"` | Defines operator intervention gates for tool execution. Valid variants:<br>• `acceptEdits` / `accept-edits`: Non-mutating reads and safe edits run automatically; destructive mutations request approval.<br>• `dont-ask`: Autonomous execution; suppresses operator confirmation prompts.<br>• `auto`: Standard automated execution governed by hook policies.<br>• `plan`: Read-only discovery mode; file mutations and commands are paused for confirmation. |
| `commandExecutionPolicy` | String enum | `"off"` | Host process execution containment. Valid variants:<br>• `off`: Direct execution prohibited unless mediated through declarative MCP tools.<br>• `bwrap_isolated`: Enforces Bubblewrap sandbox jail with read-only root and private mounts.<br>• `direct_host`: Unsandboxed host execution (reserved for local debugging). |

### 3. Orchestration & Delegation

| Field | Type | Default | Description & Valid Variants |
| :--- | :--- | :--- | :--- |
| `allowed_subagents` | Array of strings | `[]` | Whitelist of persona names that this agent is permitted to invoke via `invoke_subagent`. If empty, delegation to subagents is blocked. |
| `pre_launch_checks` | Array of strings | `[]` | Array of quality gate identifiers (from `project.toml` or native hooks) that must pass with exit code 0 before this agent can be spawned.<br>Aliases: `preLaunchChecks`, `gatekeeper_checks`.<br>Examples: `["vcs_conflicts", "compilation", "clean_working_copy"]`. |

### 4. Tool Access & MCP Server Scoping

| Field | Type | Default | Description & Valid Variants |
| :--- | :--- | :--- | :--- |
| `tools` | Array of strings | `[]` | Whitelist of host IDE built-in native tools exposed to the agent. Valid variants:<br>• `search_web`: External search engine access.<br>• `ask_question`: Interactive operator multiple-choice prompts.<br>• `invoke_subagent`: Subagent spawning engine.<br>• `read_file`: Built-in file reader.<br>• `write_file`: Built-in file writer.<br>• `edit_file`: Built-in block editor.<br>Aliases: `native_tools`. |
| `inheritMcp` | Boolean | `true` | When `false`, the agent is isolated from host-level MCP servers and receives only the MCP servers declared explicitly in its manifest. Prevents accidental tool pollution. |
| `mcpServers` | Array of objects | `[]` | Declarative list of isolated MCP server processes spawned specifically for this persona. Each entry defines:<br>• `name`: Server identifier.<br>• `command`: Binary executable name.<br>• `args`: Command-line arguments. |
| `mcp_tools` | Array of strings | `[]` | Explicit whitelist of tool names exposed by `agy-mcp` to this persona. If specified, any MCP tool not in this list is hidden from the agent's schema catalog.<br>Aliases: `mcpTools`. |

---

## Filesystem Confinement & Guard Boundaries

The agent manifest provides declarative boundary guards enforced by `agy-hooks` during every tool call (`PreToolUse`):

```yaml
allowed_dirs:
  - src
  - crates
disallowed_dirs:
  - tests
  - docs
allowed_files:
  - Cargo.toml
disallowed_files:
  - .env
  - ARCHITECTURE.md
```

### Boundary Fields

| Field | Type | Default | Enforcement Behavior |
| :--- | :--- | :--- | :--- |
| `allowed_dirs` | Array of strings | `[]` | Whitelist of workspace-relative directory paths. If non-empty, file read and edit tools targeting paths outside these directories are rejected with `deny`. Aliases: `allowedDirs`. |
| `disallowed_dirs` | Array of strings | `[]` | Blacklist of workspace-relative directory paths. Mutations or accesses targeting these trees are strictly blocked. Aliases: `disallowedDirs`. |
| `allowed_files` | Array of strings | `[]` | Whitelist of explicit individual file paths. Overrides broader directory restrictions when specified. Aliases: `allowedFiles`. |
| `disallowed_files` | Array of strings | `[]` | Blacklist of explicit individual file paths. File modifications targeting these filenames are unconditionally denied. Aliases: `disallowedFiles`. |

---

## Fine-Grained Content Filtering (`path_policies`)

Beyond coarse directory-level boundaries, `path_policies` enforces mathematical invariant content filtering on source code files:

$$\text{strip\_allowed\_domain}(\text{original}) \equiv \text{strip\_allowed\_domain}(\text{new\_content})$$

This prevents agents from altering production logic when their assigned role is documentation or test authoring.

### Schema Structure

```yaml
path_policies:
  - path: src
    filter: comments_only
  - path: tests/integration
    filter: delimited
    start_marker: "// BEGIN AGENT REGION"
    end_marker: "// END AGENT REGION"
  - path: crates/core
    filter: tests_only
    line_comments: ["//"]
    test_attributes: ["#[test]", "#[tokio::test]"]
    test_prefixes: ["test_"]
```

### Policy Subfields & Variants

| Subfield | Type | Default | Description & Valid Variants |
| :--- | :--- | :--- | :--- |
| `path` | String | *Required* | Relative directory path or file pattern to match (e.g., `"src"`, `"crates/core/lib.rs"`). |
| `filter` | String enum | `"comments_only"` | The content filter invariant enforced on matching files:<br>• `comments_only` / `comments`: The agent may modify **only** comments, docstrings (`///`, `//!`, `/* */`), and formatting whitespace. Production AST nodes cannot be altered.<br>• `tests_only` / `tests`: The agent may modify **only** unit test blocks, test functions, and test items annotated by test attributes. Production logic is immutable.<br>• `delimited`: The agent may modify **only** lines bounded strictly between `start_marker` and `end_marker`. |
| `start_marker` | String | `None` | Mandatory for `delimited` filter. The opening boundary delimiter comment (e.g., `"// BEGIN GENERATED"`). |
| `end_marker` | String | `None` | Mandatory for `delimited` filter. The closing boundary delimiter comment (e.g., `"// END GENERATED"`). |
| `line_comments` | Array of strings | Auto-inferred | Language line comment prefixes (e.g., `["//"]` for Rust/C, `["#"]` for Python, `["--"]` for Lua). Overrides language defaults. |
| `test_attributes` | Array of strings | Auto-inferred | Attribute prefixes identifying test blocks (e.g., `["#[test]"]`, `["@pytest"]`). |
| `test_prefixes` | Array of strings | Auto-inferred | Function or item name prefixes identifying tests (e.g., `["test_"]`, `["Test"]`). |

---

## Directives Body & Best Practices

The content following the closing `---` fence defines the persona's cognitive directives. Follow these guidelines when authoring directives:

1. **Explicit Negative Constraints:** Clearly specify what the persona must *never* do (e.g., *"Never edit files in `tests/`"*, *"Never delete comments or docstrings"*).
2. **Deterministic Completion Criteria:** State the exact condition required to conclude the task (e.g., *"Run `cargo check` and conclude only when exit code is 0 and compiler warnings are 0"*).
3. **Workspace Invariant Awareness:** Remind the agent that it operates inside an ephemeral sparse workspace (`.agy/workspaces/<name>/`) and that relative paths are rooted to its worktree.

---

## Discovery & Precedence

Agent manifests are discovered using the following deterministic search order:

1. `<workspace>/agents/<name>.md` *(Highest precedence: per-project custom personas)*
2. `<workspace>/plugins/agy-suite/agents/<name>.md` *(Project plugin tree)*
3. `<workspace>/.agents/agents/<name>.md`
4. `<workspace>/.agents/<name>.md`
5. `<workspace>/.gemini/agents/<name>.md`
6. `<workspace>/.agy/agents/<name>.md`
7. `~/.gemini/config/plugins/agy-suite/agents/<name>.md` *(User global plugin install)*
8. `~/.gemini/agents/<name>.md`
9. `~/.agents/agents/<name>.md`
10. `~/.agents/<name>.md` *(Lowest precedence)*

---

## The Zero-Skill Policy (`NO SKILL.md`) & Slash Commands

### Why Skills Are Banned
Google Antigravity natively supports interpreted Markdown skills (`SKILL.md`). In production systems engineering, this approach exhibits severe defects:
- **Token Inefficiency:** Injecting multi-kilobyte markdown definitions burns 2,000–4,000+ baseline tokens per conversational turn.
- **Unconfined Injection Surface:** Bash-based skills execute in ambient shells without argument bounds, timeouts, or output truncation.
- **`agy-suite` Solution:** All agent capabilities are compiled as native MCP handlers or bounded static commands with strict JSON Schemas, reducing prompt overhead to zero baseline tokens and guaranteeing deterministic execution bounds.

### High-Value Slash Commands Triage
When operating interactively with Antigravity (`agy`), use these client-side macros to maintain high context hygiene:
- `/compact`: Compresses the context window when interactive sessions get long.
- `/plan`: Switches the session into read-only plan mode, preventing premature file edits.
- `/clear`: Clears terminal history for visual decluttering.
- `/exit`: Exits the session cleanly.
