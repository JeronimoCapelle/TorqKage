# Declarative MCP Daemon (`tkg-mcp`)

`tkg-mcp` is a persistent JSON-RPC 2.0 stdio server providing harness-agnostic, language-agnostic declarative tool execution for any MCP-compatible agent client (Claude Code, OpenCode, Cursor, Google Antigravity, or custom runners).

## Native Action Handlers

Instead of shelling out to generic scripts, `tkg-mcp` compiles high-assurance native action handlers:

| Action | Description | Guards Enforced |
| :--- | :--- | :--- |
| `file_read` | Bounded file reader with line ranges and UTF-8 truncation | Path traversal, directory rejection |
| `file_write` | Atomic atomic file creation and overwriting | Path policies, dangerous files |
| `mpatch_edit` | Precision search-and-replace editor | Aider block rejection, scoped paths |
| `static_command` | Pre-configured shell command with zero arguments | Bubblewrap container isolation |
| `parameterized_command` | Command with strictly validated regex arguments | Shell metacharacter injection blocking |
| `cli_manual` | Ground-truth dynamic tool parameter inspector | In-memory schema reflection |
| `project_state` | Summarizes git/jj status and repository specs | Read-only |
| `workspace_ensure` | Ensures isolated Jujutsu subagent workspace | Jujutsu repository boundary |

## Dynamic Agent Scoping

`tkg-mcp` dynamically discovers the calling agent via `.tkg/state/active_agent` and evaluates its permissions against the `mcp_tools:` frontmatter defined in `plugins/torqkage/agents/<name>.md`. If an unconfigured or unauthorized agent calls a tool, the daemon rejects execution with an explicit policy violation error.

---

## The Meta-MCP Pattern vs. Traditional MCP Servers

In the standard MCP ecosystem, every new tool requires writing, compiling, and running a bespoke Node.js or Python process. `TorqKage` replaces this sprawl with a single, high-performance Safe Rust daemon (`tkg-mcp`). By declaring tools in `tools.toml`, developers gain instant access to:
- **Automatic JSON Schema Synthesis:** Generates strict type schemas from TOML declarations.
- **Regex Argument Validation:** Intercepts shell injections before any subprocess is spawned.
- **Bubblewrap Container Isolation:** Runs untrusted commands inside private namespaces (`CLONE_NEWNET`, private loopback only, zero outbound sockets).
- **UTF-8 Output Truncation:** Caps large command outputs (`truncate_lines`) to protect the model's context window.


