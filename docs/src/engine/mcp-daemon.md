# Declarative MCP Daemon (`agy-mcp`)

`agy-mcp` is a persistent JSON-RPC 2.0 stdio server providing declarative tool execution for Google Antigravity.

## Native Action Handlers

Instead of shelling out to generic scripts, `agy-mcp` compiles high-assurance native action handlers:

| Action | Description | Guards Enforced |
| :--- | :--- | :--- |
| `file_read` | Bounded file reader with line ranges and UTF-8 truncation | Path traversal, directory rejection |
| `file_write` | Atomic atomic file creation and overwriting | Path policies, dangerous files |
| `mpatch_edit` | Precision search-and-replace editor | Aider block rejection, scoped paths |
| `static_command` | Pre-configured shell command with zero arguments | Bubblewrap container isolation |
| `parameterized_command` | Command with strictly validated regex arguments | Shell metacharacter injection blocking |
| `cargo_json_diagnostics` | Runs `cargo check` and parses compiler error spans | Workspace boundary |
| `cli_manual` | Ground-truth dynamic tool parameter inspector | In-memory schema reflection |
| `project_state` | Summarizes git/jj status and repository specs | Read-only |
| `workspace_ensure` | Ensures isolated Jujutsu subagent workspace | Jujutsu repository boundary |

## Dynamic Agent Scoping

`agy-mcp` dynamically discovers the calling agent via `.agy/state/active_agent` and evaluates its permissions against the `mcp_tools:` frontmatter defined in `plugins/agy-suite/agents/<name>.md`. If an unconfigured or unauthorized agent calls a tool, the daemon rejects execution with an explicit policy violation error.
