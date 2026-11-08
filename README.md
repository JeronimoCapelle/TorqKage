# agy-suite

[![CI](https://github.com/JeronimoCapelle/agy-suite/actions/workflows/ci.yml/badge.svg?branch=main)](https://github.com/JeronimoCapelle/agy-suite/actions/workflows/ci.yml)
[![Rust Edition](https://img.shields.io/badge/rust-2024%20edition-orange.svg)](https://www.rust-lang.org/)
[![License: AGPL-3.0-or-later](https://img.shields.io/badge/License-AGPL--3.0--or--later-blue.svg)](LICENSE)
[![Documentation](https://img.shields.io/badge/docs-manual-blue.svg)](https://jeronimocapelle.github.io/agy-suite/)
[![MCP Protocol](https://img.shields.io/badge/MCP-2024--11--05-green.svg)](https://modelcontextprotocol.io/)
[![Ratatui](https://img.shields.io/badge/Ratatui-0.30-purple.svg)](https://ratatui.rs/)

### Sandboxed Meta-MCP Runtime and Declarative Fleet Engine
Turn any CLI binary, script, or compiler into a sandboxed, schema-validated MCP tool with declarative TOML configurations.

Built in Safe Rust (`#![forbid(unsafe_code)]`) with Bubblewrap container isolation, sub-millisecond lifecycle hooks (<1ms, <500KB RSS), and an interactive Ratatui terminal dashboard.

<p align="center">
  <img src="assets/demo.gif" alt="agy-suite Interactive Terminal Control Deck Demo" width="100%" />
</p>

---

## About agy-suite

Running autonomous AI coding agents with direct shell access carries real operational risks: destructive commands, credential exposure, unintended network access, dirty git worktrees, and high token costs from repeatedly injecting CLI documentation into model context.

`agy-suite` provides declarative containerization and governance for AI agent fleets. Written in Safe Rust (`#![forbid(unsafe_code)]`), it functions as a Meta-MCP server: any CLI binary, script, or compiler declared in TOML becomes an isolated, schema-validated MCP tool with Bubblewrap container sandboxing, sub-millisecond lifecycle hooks, and per-agent permission allowlists.

The repository includes a 10-agent reference fleet for systems engineering, pre-built starter configurations for Go, Python, TypeScript, and Rust, and an interactive terminal dashboard (`agy-ctrl`) for managing permissions, inspecting tools, and resuming sessions.

---

## Why agy-suite

Configuring tool access for autonomous coding agents usually involves an undesirable choice:

1. **MCP sprawl:** Writing and running separate Node.js or Python MCP servers for every linter, compiler, and CLI tool, which increases resource usage and maintenance overhead.
2. **Ambient shell access:** Giving agents direct terminal access, which risks unintended filesystem changes, secret exposure, and burns context tokens repeatedly re-explaining CLI usage.

`agy-suite` bridges this gap. It is a declarative Meta-MCP server and agent engine built for [Google Antigravity (`agy`)](https://github.com/google/antigravity). Tools declared in `tools.toml` are exposed as structured MCP actions governed by strict security policies:

| Problem | How agy-suite Addresses It |
| :--- | :--- |
| **Unrestricted shell execution:** Agents run arbitrary commands with host user privileges. | **Bubblewrap container sandboxing:** Tools declared with `enforce_sandbox = true` run in an isolated Bubblewrap container with private loopback networking and read-only host mounts. |
| **Token overhead:** Long prompt descriptions and markdown guides consume context on every interaction. | **Native interceptors (<1ms, <500KB RSS):** Schema validation, argument checking, and output filtering happen in native code before reaching model context. |
| **Lost session context:** Disconnected terminal sessions lose state, requiring manual restarts. | **Session resumption:** Embedded SQLite state tracking allows resuming prior sessions directly from `agy-ctrl` (`Enter` / `r`). |
| **Working tree pollution:** Agent modifications clutter the primary working directory and editor buffers. | **Jujutsu workspace isolation:** Agents work in dedicated, sparse Jujutsu working copies (`.agy/workspaces/<agent>`) without modifying the active working copy. |
| **Failing command loops:** Agents repeat the same failing compiler or test command indefinitely. | **Circuit breaker:** Execution halts after consecutive command failures until reviewed or reset (`c`). |

---

## Quickstart

### 1. Precompiled Binaries
Install the engine and register the Antigravity plugin:

```bash
curl -sSL https://raw.githubusercontent.com/JeronimoCapelle/agy-suite/main/install.sh | bash
```

### 2. Build From Source
To compile from source:

```bash
git clone https://github.com/JeronimoCapelle/agy-suite.git
cd agy-suite
./install.sh --perf       # Production profile (LTO enabled, stripped)
```
*(For faster local compilation with Cranelift and Mold, pass `--fast`.)*

### 3. Launch the Control Deck
Run the interactive terminal dashboard:

```bash
agy-ctrl
```

---

## Usage Guide

### 1. Terminal Dashboard (`agy-ctrl`)
`agy-ctrl` provides a terminal interface to inspect tools, configure permission gates, monitor sessions, and launch agents:

| Keybinding | Action |
| :--- | :--- |
| <kbd>Enter</kbd> / <kbd>r</kbd> | **Resume session / Launch agent:** Resume the selected SQLite session or launch the active agent in Antigravity |
| <kbd>b</kbd> / <kbd>F8</kbd> | **Toggle Bubblewrap sandbox:** Switch between host execution and jailed Bubblewrap containment |
| <kbd>c</kbd> / <kbd>Shift-K</kbd> | **Reset circuit breaker:** Clear tripped failure counts in `.agy/state/` |
| <kbd>F2</kbd> | **Save & validate:** Validate agent frontmatter against schemas in real time |
| <kbd>Tab</kbd> / <kbd>Shift-Tab</kbd> | Switch panels between the Agent Fleet Matrix, Tool Catalog, and Sessions |
| <kbd>?</kbd> | Open the embedded manual and cheat sheet |
| <kbd>q</kbd> / <kbd>Esc</kbd> | Exit `agy-ctrl` |

### 2. Launching Agents
Invoke configured agents from the command line:

```bash
agy --agent coder
agy --agent test-author
agy --agent reviewer
```

### 3. Build Acceleration
Standard stable Rust is used by default. If your environment has Cranelift, Mold, or `sccache` installed, use the included helper script:

```bash
./plugins/agy-suite/bin/cargo-fast build
./plugins/agy-suite/bin/cargo-fast test
```

---

## Ecosystem Starter Kits

Pre-configured starter setups are available in [`docs/templates/`](docs/templates/):

| Starter Kit | Target Ecosystem | Tooling & Sandboxing Highlights | Agent Team |
| :--- | :--- | :--- | :--- |
| **[Go Microservices](https://jeronimocapelle.github.io/agy-suite/templates/go-setup.html)** | Go 1.22+ (`cmd/`, `internal/`) | Sandboxed `go test`, enum flags (`-short`, `-race`), migration permission gates | `coder`, `reviewer` |
| **[Python & Data Engineering](https://jeronimocapelle.github.io/agy-suite/templates/python-setup.html)** | Python 3.12+ (uv, pytest, ruff, mypy) | Sandboxed `pytest` with regex filters, Mypy typing, automated Ruff fixer | `coder`, `tester` |
| **[TypeScript & Fullstack Web](https://jeronimocapelle.github.io/agy-suite/templates/typescript-setup.html)** | Node.js / React / Next.js (pnpm, Vitest, tsc) | Sandboxed `vitest` with target scoping, `tsc --noEmit` gate, ESLint check | `frontend`, `reviewer` |
| **[Standard Rust Systems](https://jeronimocapelle.github.io/agy-suite/templates/rust-setup.html)** | Rust 2024 Edition (Cargo workspaces) | Sandboxed `cargo test`, enum targets (`--lib`, `--bins`), Clippy gate | `coder`, `doc-author` |
| **[Adversarial QA Auditor](https://jeronimocapelle.github.io/agy-suite/templates/qa-tester.html)** | Ecosystem-agnostic | Fuzz testing with LLVM sanitizers (enums), schema verification | `tester` (`tester.md`) |

To use a starter kit, copy its configuration into `.agy/`:

```bash
mkdir -p .agy
cp -r docs/templates/go/* .agy/
agy-ctrl
```

---

## Declarative Fleets from Prompts

Because `agy-suite` configurations are declarative TOML and Markdown, an LLM can generate a customized fleet setup from schemas:

```text
"Act as an agy-suite Fleet Architect. I am building a project in <Language/Framework>.
Based on the agy-suite declarative schemas, generate:
1. project.toml: with gates for compilation, unit tests, and linting.
2. tools.toml: declaring test runners and diagnostic tools with enforce_sandbox = true,
   strict regex parameter validation, and enums for mutually exclusive flags.
3. hooks.toml: with a 2-strike compiler circuit breaker.
4. agents/: 2 specialized agent personas with clean allowed_dirs and mcp_tools allowlists."
```

Place the generated files in `.agy/` and run `agy-ctrl` to activate the configuration.

---

## The 10-Agent Reference Fleet

The reference fleet in `plugins/agy-suite/` follows a Design Verification (DV) discipline where implementation engineers do not author their own testbenches:

| Agent Profile | Role & Specialty | Permitted Directory Scope | Content & Security Policy |
| :--- | :--- | :--- | :--- |
| **[`coder`](plugins/agy-suite/agents/coder.md)** | Systems implementer (Rust, RISC-V asm) | `src/`, `crates/`, `boards/`, `targets/` | Full code mutation; `Cargo.toml` protected by `manifest_guard` |
| **[`test-author`](plugins/agy-suite/agents/test-author.md)** | Design Verification (DV) & Formal Test Engineer | `tests/`, `benches/`, `src/` | `tests_only` in `src/` (`mod tests`); formal proofs (Kani, Loom, Miri) |
| **[`doc-author`](plugins/agy-suite/agents/doc-author.md)** | Specification & Rustdoc Author | `docs/`, `specs/`, `README.md`, `ARCHITECTURE.md` | `comments_only` in `src/` (in-place `///` rustdoc without logic edits) |
| **[`planner`](plugins/agy-suite/agents/planner.md)** | Architecture & Verification Strategy | `docs/`, `ARCHITECTURE.md` | Authoritative architecture RFCs and upfront verification planning |
| **[`copilot`](plugins/agy-suite/agents/copilot.md)** | Workflow Coordinator | Read-only across repository | Read-only code inspection; Jujutsu workspace rebase and merge |
| **[`finisher`](plugins/agy-suite/agents/finisher.md)** | Release Gatekeeper | Read-only across repository | Pre-release checks (fmt, clippy, tests, diffs) |
| **[`spec-checker`](plugins/agy-suite/agents/spec-checker.md)** | Spec & ISA Compliance Auditor | Read-only across repository | AST queries, opcode audits, and spec-to-source traceability |
| **[`perf-analyst`](plugins/agy-suite/agents/perf-analyst.md)** | Performance & Benchmark Engineer | `benches/`, `docs/`, `.agy/artifacts/` | Microbenchmarking (Divan/Criterion), flamegraphs, `llvm-mca` |
| **[`mini`](plugins/agy-suite/agents/mini.md)** | Symbolic Math Specialist | Strictly air-gapped | Network-isolated Bubblewrap container using Python/SymPy |
| **[`sysadmin`](plugins/agy-suite/agents/sysadmin.md)** | Linux System Administrator | Full system | Supervised shell; requires operator confirmation on mutating actions |

---

## Configuration

Tools and agents can be added or modified without recompiling binaries:

### 1. Adding a Tool in `tools.toml`
Declare tools in [`plugins/agy-suite/tools.toml`](plugins/agy-suite/tools.toml) or `.agy/tools.toml`:

```toml
[tools.my_scanner]
inherit = "base_tools.cargo_base"
description = "Executes security scan with regex-validated arguments"
action = "parameterized_command"
command = ["cargo", "audit"]
enforce_sandbox = true
mutating = false
args = [
    { name = "target", required = false, flag = "--target", regex = "^[a-zA-Z0-9_-]+$", description = "Crate target name" },
    { name = "mode", required = false, default = "advisories", enum = ["advisories", "bans", "licenses"], description = "Audit check mode" }
]
```

### 2. Defining an Agent
Create [`plugins/agy-suite/agents/my-agent.md`](plugins/agy-suite/agents/) or `.agy/agents/my-agent.md`:

```markdown
---
name: my-agent
description: "Specialized auditing agent"
effort: high
mainAgent: true
permissionMode: dont-ask
allowed_dirs: ["src", "tests"]
mcp_tools:
  - read_project_file
  - search_codebase
  - my_scanner
pre_launch_checks:
  - compilation
  - vcs_conflicts
---

You are the specialized auditing agent for this repository.
```

Saved agents appear in `agy-ctrl` with full gate enforcement.

---

## Documentation

For architecture details, system invariants, and guides:

* **[Canonical System Architecture (`ARCHITECTURE.md`)](ARCHITECTURE.md)**:
  * Invariants and error handling guarantees.
  * Declarative contracts (`tools.toml`, `hooks.toml`, `project.toml`).
  * Process tree supervision and termination handling (`SubprocessSupervisor`).
  * Bubblewrap container sandboxing and loopback network model.
  * Design Verification (DV) methodology.
* **[The agy-suite Engine Manual](https://jeronimocapelle.github.io/agy-suite/)**:
  * [Pre-Built Ecosystem Starter Kits](https://jeronimocapelle.github.io/agy-suite/templates/overview.html)
  * Setup guides and workflow tutorials.
  * Verification flow and lifecycle hook details.
  * Performance characteristics and resource footprint.

---

## License

Licensed under the **GNU Affero General Public License v3.0 or later** ([AGPL-3.0-or-later](LICENSE)).
