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
| `cli_manual` | Ground-truth dynamic tool parameter inspector | In-memory schema reflection |
| `project_state` | Summarizes git/jj status and repository specs | Read-only |
| `workspace_ensure` | Ensures isolated Jujutsu subagent workspace | Jujutsu repository boundary |

## Dynamic Agent Scoping

`agy-mcp` dynamically discovers the calling agent via `.agy/state/active_agent` and evaluates its permissions against the `mcp_tools:` frontmatter defined in `plugins/agy-suite/agents/<name>.md`. If an unconfigured or unauthorized agent calls a tool, the daemon rejects execution with an explicit policy violation error.

---

## The Meta-MCP Pattern vs. Traditional MCP Servers

In the standard MCP ecosystem, every new tool requires writing, compiling, and running a bespoke Node.js or Python process. `agy-suite` replaces this sprawl with a single, high-performance Safe Rust daemon (`agy-mcp`). By declaring tools in `tools.toml`, developers gain instant access to:
- **Automatic JSON Schema Synthesis:** Generates strict type schemas from TOML declarations.
- **Regex Argument Validation:** Intercepts shell injections before any subprocess is spawned.
- **Bubblewrap Container Isolation:** Runs untrusted commands inside private namespaces (`CLONE_NEWNET`, private loopback only, zero outbound sockets).
- **UTF-8 Output Truncation:** Caps large command outputs (`truncate_lines`) to protect the model's context window.

---

## Curated Third-Party MCP Reference Patterns & Evaluations

When evaluating external tools or building specialized plugins, `agy-suite` maintains a strict evaluation standard:

| Category | Reference Tool | Lang / Target | Evaluated Pattern | Architecture Verdict |
| :--- | :--- | :--- | :--- | :--- |
| **Code Graph** | [`graphtrail`](https://github.com/ast-grep/graphtrail) | Rust / CLI | Tree-sitter AST call graphs, callers/callees, blast radius | **Recommended Host Tool** (discovered via `$PATH`) |
| **System Tracing** | [`perfetto-mcp-rs`](https://github.com/tooluse-labs/perfetto-mcp-rs) | Rust / Stdio | PerfettoSQL queries on `.pftrace` files | **Recommended Host Tool** (discovered via `$PATH`) |
| **Code Intel** | [`quarry`](https://github.com/sait-turanalp/quarry) | Rust / Local | Language-agnostic ASTs and impact analysis across 14 languages | **Top Candidate** for structural code navigation |
| **Search Bridge** | [`mcp-ripgrep`](https://github.com/mcollina/mcp-ripgrep) | Native / Stdio | Ripgrep regex, glob, and line-slice search wrapper | **Reference Pattern** for binary stdio streaming |
| **Headless Browser** | [`browser-use-rs`](https://github.com/BB-fat/browser-use-rs) | Rust / Local | Pure Rust DevTools protocol driver; zero Node.js/Playwright | **Reference Design** for lightweight web testing |
| **Unified Storage** | [`mcp-server-opendal`](https://github.com/Xuanwo/mcp-server-opendal) | Rust / OpenDAL | Unified storage abstraction across POSIX, memory, object stores | **Reference Pattern** for cross-filesystem caching |
| **Local SQL** | [`anyquery`](https://github.com/julien040/anyquery) | Go / Native | Single binary running ANSI SQL across CSV, JSON, Git, SQLite | **High Utility** for tabular log analysis |
| **Canned SQL** | [`mcp-sqlite`](https://github.com/panasenco/mcp-sqlite) | Python / SQLite | Parameterized read-only queries exposed as separate tools | **Security Pattern** adopted for bounded DB access |
| **Security Audit** | [`aguara`](https://github.com/garagon/aguara) | Rust / CLI | Static scanner for MCP definitions and prompt injection vectors | **Security Scanner** for schema verification |
| **Kernel Sandbox** | [`assay`](https://github.com/Rul1an/assay) | Rust / Linux | Landlock LSM + eBPF socket filters for agent runners | **Study Implementation** for future Landlock integration |

---

## Architectural Exclusion Standards

To prevent performance degradation and supply-chain vulnerabilities, `agy-suite` formally excludes:
1. **SaaS Cloud Wrappers:** Notion, Slack, Jira, Twitter/X (violates local-first, zero-cloud security bounds).
2. **Monolithic Node.js Wrappers:** Avoid packaging heavy Node.js runtimes for simple CLI commands. Prefer native compiled binaries (`tools.toml`).
3. **Speculative Agent Economies:** Crypto or escrow tools with untrusted third-party dependencies.
