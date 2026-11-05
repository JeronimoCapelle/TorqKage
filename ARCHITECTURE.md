# agy-suite: System Architecture & Invariant Manual

**Document Version:** 1.0.0  
**Status:** Canonical & Permanent Architecture Specification  
**Authority:** Root repository manual for human developers and autonomous AI agents (`coder`, `planner`, `spec-checker`, `finisher`, `copilot`).

---

## 1. System Invariants & Non-Negotiable Directives

All autonomous agents and human contributors operating within this repository or deploying `agy-suite` MUST adhere to the following invariants:

1. **Manifest Integrity (Strict Prohibition):**
   - **NEVER** autonomously edit `Cargo.toml`, `Cargo.lock`, or `deny.toml`.
   - Modifying dependencies or build configurations without human review is prohibited to prevent dependency bloat, accidental malicious package injection, and proc-macro execution vectors.
   - If a new dependency is required, emit a structured request to `.agy/state/dependency_requests.json` and notify the operator.
2. **Compiler & Linter Cleanliness:**
   - The codebase compiles under `#![deny(clippy::all)]` and `#![deny(clippy::pedantic)]`.
   - **Zero compiler warnings and zero clippy warnings** are tolerated across all workspace targets (`--all-targets`).
   - Every `#[allow(...)]` attribute requires an explicit `reason = "..."` explanation. Blanket crate-level lint suppressions are prohibited.
3. **Robust Error Handling:**
   - **Zero unhandled `unwrap()` or `expect()` calls** in production code. All errors must use typed enumerations (`thiserror`) or return descriptive `Result<T, String>` / `Result<T, AgyError>`.
4. **Subprocess Supervision & Hang Immunity:**
   - **Never** invoke raw synchronous `Command::new(...).output()` without timeout bounding.
   - All external processes must execute through `SubprocessSupervisor::run_supervised` with bounded deadlines (`agy-common::limits`) to prevent daemon or IDE freezes on lock contention or hanging processes.
5. **Process Group Safety:**
   - When dispatching group termination signals (`libc::kill`), **always** assert that `pgid > 0` before negating. Never invoke `libc::kill(0, sig)` which terminates the caller's own process group.
6. **Thread Safety & Rust 2024 Compliance:**
   - **Zero** `unsafe { std::env::set_var }` or `remove_var` calls in production or test code. Parameterize environment lookups to prevent multithreaded data races.
7. **Fast Verification Standard:**
   - Use `cargo fast` (`cargo fast check`, `cargo fast clippy`, `cargo fast test`, `cargo fast build`) for rapid iteration powered by Cranelift and Mold.

---

## 2. The Three Declarative Contracts

`agy-suite` enforces a strict separation between **Platform Plumbing**, **Project Environment**, and **Agent Tooling**:

```
+-----------------------------------------------------------------------------------+
| 1. tools.toml (The Agent's Hands)                                                 |
|    - Declares MCP tools exposed to agents, base templates, actions, and limits.   |
|    - Defines per-agent blast perimeters ([agent_guards.<agent>]).                 |
+-----------------------------------------------------------------------------------+
| 2. hooks.toml (The Runtime's Rules)                                               |
|    - Declares lifecycle interception pipeline (<1ms native Rust execution).       |
|    - Circuit breaker strike logic, manifest mutation guards, conflict sentries.  |
+-----------------------------------------------------------------------------------+
| 3. project.toml (The Codebase's Identity)                                         |
|    - Declarative contracts for the target repository.                             |
|    - Compiler gates, test harness checks, workspace containers, build cache       |
|      bind mounts, documentation paths, and security allow/deny rules.             |
+-----------------------------------------------------------------------------------+
```

### Hierarchy & Fallback Rules
- Workspace configuration always takes precedence: `<workspace>/.agy/project.toml` $\to$ `<workspace>/project.toml` $\to$ `<workspace>/plugins/agy-suite/project.toml`.
- When operating in an external repository without a `project.toml`, `agy-suite` falls back to a **blank, restrictive default configuration** rather than making language assumptions.

---

## 3. Agent Fleet Perimeters & Blast Boundaries

Each agent in `plugins/agy-suite/agents/` operates under the **Air-Gapped Least-Privilege Model**:

| Agent | Responsibility | Permitted Directory Scope (`tools.toml`) | Available Tooling Scope |
| :--- | :--- | :--- | :--- |
| **`coder`** | Production implementation | `allowed_dirs = ["src"]`<br>`disallowed_dirs = ["tests", "benches", "docs"]` | Fast compiler diagnostics, `mpatch_edit`, `write_file`, `search_codebase`, `read_project_file`. |
| **`test-author`** | Comprehensive test suites | `allowed_dirs = ["tests", "benches"]`<br>`disallowed_dirs = ["src", "docs"]` | Test runners (`nextest`, `mutants`, `insta`, `fuzz`), test file writer/editor. |
| **`planner`** | Systems design & specs | `allowed_dirs = ["docs", ".scratchpad"]`<br>`allowed_files = ["ARCHITECTURE.md"]` | Read-only codebase queries, architecture document editor. |
| **`copilot`** | Interactive Chief of Staff | `allowed_dirs = ["*"]` | High-level fleet orchestration, subagent delegation, handoff inspection. |
| **`finisher`** | Release gatekeeper | `allowed_dirs = ["*"]` (Advisory) | Read-only inspection tools: git/jj diffs, public API diffs, semver checks. |
| **`spec-checker`** | Spec-vs-code consistency | **Strictly read-only** (`disallowed_dirs = ["*"]`) | Read-only spec searches, AST grep queries, clippy, cargo-deny. |
| **`perf-analyst`** | Profiling & diagnostics | `allowed_dirs = ["benches", "docs", ".agy/artifacts"]` | Profilers, Miri, flamegraphs, asm inspection, llvm-mca. |
| **`mini`** | Symbolic math & calculation | **Strictly air-gapped** (`disallowed_dirs = ["*"]`) | `eval_symbolic_math` (SymPy) inside isolated container only. |
| **`sysadmin`** | Arch Linux / CachyOS admin | `allowed_dirs = ["*"]` | Host troubleshooting, systemd, pacman inspection (all mutating ops prompt `[y/n]`). |

### Workspace Isolation
- Each agent operates in its own sparse Jujutsu workspace at `.agy/workspaces/<agent>/`.
- Agent modifications remain cleanly isolated from the operator's active `@` commit and open editor buffers.
- The shared build cache (e.g. `target/`) is symlinked to ensure compilation artifacts are shared without disk bloat.

---

## 4. Subagent Admission & Quality Gatekeeping

When `invoke_subagent` is called, `agy-hooks` evaluates the `subagent_admission` gate before the subagent can boot:
1. Target agent's frontmatter `pre_launch_checks: [...]` is extracted.
2. Gates are matched against `project.toml [gates.<name>]` (e.g. `compilation`, `tests`).
3. If any gate check fails:
   - Admission is **denied**.
   - The subagent is not launched, preventing wasted LLM tokens.
   - Diagnostic output is returned to the calling agent to guide remediation.

---

## 5. Security & Filesystem Confinement

Enforced across all tools (`read_project_file`, `write_file`, `modify_code`, `search_codebase`):

1. **Path Traversal Defense:** Absolute paths outside the workspace and relative escapes (`../`) are rejected.
2. **Sensitive File Protection:**
   - Generic credentials (`password`, `secret`, `token`, `credential`, `api_key`).
   - VCS internals (`.git`, `.jj`, `.hg`, `.svn`).
   - SSH private keys (`id_rsa`, `id_ed25519`, `id_ecdsa`).
   - Shell profiles (`.bashrc`, `~/.zshrc`, `.profile`).
   - Sensitive cryptographic extensions (`.key`, `.pem`, `.pfx`, `.p12`, `.keystore`).
   - Configurable per-project via `project.toml [security]`.
3. **Manifest Mutation Guard:**
   - Intercepts mutating tools targeting build scripts (`build.rs`, `Cargo.toml`, `package.json`, `CMakeLists.txt`, `Makefile`).
   - Neutralizes procedural macro and build-script host code execution vulnerabilities.

---

## 6. Directory Layout of the Repository

```
agy-suite/
├── ARCHITECTURE.md     # Canonical system architecture and invariant manual (this file)
├── README.md           # Repository overview and quickstart guide
├── Cargo.toml          # Workspace root manifest (DO NOT EDIT AUTONOMOUSLY)
├── deny.toml           # Supply-chain security and license policy (DO NOT EDIT)
├── install.sh          # Self-contained bundle builder & Gemini plugin installer
├── crates/
│   ├── agy-common/     # Core domain models, project config, supervisor, Jujutsu, state
│   ├── agy-mcp/        # Declarative MCP server, tool execution engine, agent guards
│   ├── agy-hooks/      # High-assurance lifecycle interceptors (<1ms native execution)
│   └── agy-ctrl/       # Ratatui 0.29 TUI dashboard, fleet matrix, and installer
└── plugins/
    └── agy-suite/      # Self-contained Antigravity plugin bundle
        ├── plugin.json # Host manifest
        ├── hooks.json  # Host lifecycle hook bindings
        ├── mcp_config.json # Host MCP stdio configuration
        ├── project.toml# Declarative project contracts and multi-language scaffolds
        ├── hooks.toml  # Lifecycle interception pipeline and circuit breaker rules
        ├── tools.toml  # Declarative MCP tool catalog and agent perimeters
        ├── README.md   # Plugin documentation and customization cookbook
        └── agents/     # Fleet specifications (coder, copilot, finisher, etc.)
```
