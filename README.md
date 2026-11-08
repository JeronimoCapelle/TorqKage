# agy-suite

[![CI](https://github.com/JeronimoCapelle/agy-suite/actions/workflows/ci.yml/badge.svg?branch=main)](https://github.com/JeronimoCapelle/agy-suite/actions/workflows/ci.yml)
[![Rust Edition](https://img.shields.io/badge/rust-2024%20edition-orange.svg)](https://www.rust-lang.org/)
[![License: AGPL-3.0-or-later](https://img.shields.io/badge/License-AGPL--3.0--or--later-blue.svg)](LICENSE)
[![Documentation](https://img.shields.io/badge/docs-manual-blue.svg)](https://jeronimocapelle.github.io/agy-suite/)
[![MCP Protocol](https://img.shields.io/badge/MCP-2024--11--05-green.svg)](https://modelcontextprotocol.io/)
[![Ratatui](https://img.shields.io/badge/Ratatui-0.30-purple.svg)](https://ratatui.rs/)

### Stop writing MCP servers. agy-suite is the last MCP runtime you'll ever need.
**Turn any CLI binary, script, or compiler into an air-gapped, schema-validated MCP tool in 6 lines of TOML—and compose governed autonomous agent fleets in seconds.**

*Zero Node.js bloat. Zero token-burning bash wrappers. 100% Safe Rust (`#![forbid(unsafe_code)]`) with Bubblewrap container sandboxing, sub-millisecond lifecycle hooks (<1ms, <500KB RSS), and an interactive Ratatui 0.30 flight deck.*

<p align="center">
  <img src="assets/demo.gif" alt="agy-suite Interactive Terminal Control Deck Demo" width="100%" />
</p>

---

## 🎯 What is agy-suite?

Building tool environments for autonomous coding agents today forces developers into an awful tradeoff:
1. **The MCP Sprawl:** You write, debug, and run dozens of bespoke Node.js or Python MCP servers for every single linter, test runner, and CLI tool—bloating memory and fighting port collisions.
2. **The Ambient Shell Trap:** You give agents unconstrained `bash` access—risking accidental `rm -rf`, prompt injections, credential leaks, and burning 2,000–5,000 tokens on *every single turn* re-injecting tool documentation.

`agy-suite` eliminates this tradeoff entirely. It is a **Meta-MCP Server and Declarative Fleet Engine** built for [Google Antigravity (`agy`)](https://github.com/google/antigravity) in pure Safe Rust. Any CLI tool, compiler, or script declared in `tools.toml` instantly becomes an air-gapped, schema-validated MCP action governed by declarative contracts:

| The Raw AI Coding Problem | How the `agy-suite` Engine Solves It |
| :--- | :--- |
| **Ambient Shell Poisoning:** Agents execute uncontrolled `bash` commands with host authority, risking accidental `rm -rf`, network leaks, or credential theft. | 🛡️ **Container-Grade Bubblewrap Sandboxing:** Declare `enforce_sandbox = true` on any tool in `tools.toml` to run in an isolated Bubblewrap jail with private loopback, unshared network, and read-only host mounts. |
| **Token Exhaustion:** Inlining multi-kilobyte prompt instructions, markdown skills, and manuals burns 2,000–5,000 baseline tokens on *every single turn*. | ⚡ **Compiled Native Interceptors (<1ms, <500KB RSS):** Offline Rust daemons handle parameter schemas, regex validation, and output slicing with **zero baseline prompt token burn**. |
| **Lost Conversational State:** Terminal drops or shell disconnects lose the active conversational thread, forcing you to re-prompt from scratch. | 🔄 **Multi-Source Session Resumption:** Embedded SQLite & history correlation automatically surface resumable sessions directly in `agy-ctrl` (`[Enter]` / `r`). |
| **Worktree & Buffer Pollution:** Unsupervised model edits pollute your active working copy and editor buffers with uncommitted churn. | 🌿 **Jujutsu Workspace Isolation:** Agents operate in dedicated, sparse Jujutsu working copies (`.agy/workspaces/<agent>`) without ever mutating your active `@` revision. |
| **Runaway Failure Loops:** Agents hallucinate or repeat the same failing compiler fix across endless expensive API turns. | 🛑 **2-Strike Circuit Breaker:** Automatically trips after 2 consecutive compiler/test failures, halting execution until human review or single-key reset (`c`). |

---

## ⚡ Quickstart

### 1. Instant Install (Precompiled Binaries)
Install the full engine and register the Antigravity plugin with a single command (downloads precompiled native binaries for your architecture; zero Rust compilation required):

```bash
curl -sSL https://raw.githubusercontent.com/JeronimoCapelle/agy-suite/main/install.sh | bash
```

### 2. Build From Source
Prefer compiling from source on your machine?

```bash
git clone https://github.com/JeronimoCapelle/agy-suite.git
cd agy-suite
./install.sh --perf       # Production perf profile (fat LTO, stripped)
```
*(Tip: For rapid sub-second local compilation with Cranelift + Mold, append `--fast`).*

### 3. Launch the Control Deck
Run the interactive flight deck in any repository:

```bash
agy-ctrl
```

---

## 🚀 Quick Usage Guide

### 1. Interactive Flight Cockpit (`agy-ctrl`)
`agy-ctrl` provides a high-assurance terminal UI to navigate policies, inspect active gates, resume past sessions, and launch agents:

| Keybinding | Action |
| :--- | :--- |
| <kbd>Enter</kbd> / <kbd>r</kbd> | **Resume Session / Launch Agent:** Resumes the selected SQLite session or launches the active agent in Antigravity |
| <kbd>b</kbd> / <kbd>F8</kbd> | **Toggle Bubblewrap Sandbox:** Seamlessly switch between host execution and jailed Bubblewrap containment |
| <kbd>c</kbd> / <kbd>Shift-K</kbd> | **Reset Circuit Breaker:** Clears tripped 2-strike compiler ledgers in `.agy/state/` |
| <kbd>F2</kbd> | **Save & Validate:** Validates agent YAML frontmatter against declarative schemas in real-time |
| <kbd>Tab</kbd> / <kbd>Shift-Tab</kbd> | Switch panels between the **Agent Fleet Matrix**, **Tool Catalog**, and **Sessions** |
| <kbd>?</kbd> | Open the embedded Engine Manual & Cheat Sheet |
| <kbd>q</kbd> / <kbd>Esc</kbd> | Exit `agy-ctrl` |

### 2. Launching Specialized Agents
You can invoke any configured agent directly from the command line:

```bash
# Launch specific personas in the active workspace
agy --agent coder
agy --agent test-author
agy --agent reviewer
```

### 3. Sub-Second Build Acceleration
`agy-suite` defaults to 100% standard stable Rust out-of-the-box. If your system has Cranelift, Mold, or `sccache` installed, use the included accelerator:

```bash
# Auto-detects Cranelift + Mold + sccache for sub-second compile cycles:
./plugins/agy-suite/bin/cargo-fast build
./plugins/agy-suite/bin/cargo-fast test
```

---

## 📦 Pre-Built Ecosystem Starter Kits

You don't need to configure everything from scratch. `agy-suite` ships with complete, copy-pasteable starter setups in [`docs/templates/`](docs/templates/):

| Starter Kit | Target Ecosystem | Included Tooling & Sandboxing Highlights | Agent Team |
| :--- | :--- | :--- | :--- |
| **[Go Microservices](https://jeronimocapelle.github.io/agy-suite/templates/go-setup.html)** | Go 1.22+ (`cmd/`, `internal/`) | Sandboxed `go test`, `enum` mode flags (`-short`, `-race`), database migration permission gates | `coder`, `reviewer` |
| **[Python & Data Engineering](https://jeronimocapelle.github.io/agy-suite/templates/python-setup.html)** | Python 3.12+ (uv / pytest / ruff / mypy) | Sandboxed `pytest` with keyword regex filters, Mypy static typing, automated Ruff fixer | `coder`, `tester` |
| **[TypeScript & Fullstack Web](https://jeronimocapelle.github.io/agy-suite/templates/typescript-setup.html)** | Node.js / React / Next.js (pnpm / Vitest / tsc) | Sandboxed `vitest` with target scoping, `tsc --noEmit` gate, ESLint zero-warning checks | `frontend`, `reviewer` |
| **[Standard Rust Systems](https://jeronimocapelle.github.io/agy-suite/templates/rust-setup.html)** | Rust 2024 Edition (Cargo workspaces) | Bubblewrap sandboxed `cargo test`, `enum` targets (`--lib`, `--bins`), Clippy `-D warnings` gate | `coder`, `doc-author` |
| **[Adversarial QA Auditor](https://jeronimocapelle.github.io/agy-suite/templates/qa-tester.html)** | Ecosystem-agnostic | Fuzz testing with LLVM sanitizers (`enum`), schema verification, unrestricted adversarial probing | `tester` (`tester.md`) |

To use any starter kit, copy it into `.agy/`:
```bash
mkdir -p .agy
cp -r docs/templates/go/* .agy/
agy-ctrl
```

---

## 🤖 The "Prompt-to-Fleet" Workflow (LLM as Fleet Architect)

Because `agy-suite` is 100% declarative, you can point Claude, ChatGPT, Gemini, or Antigravity to your schemas and prompt:

```text
"Act as an agy-suite Fleet Architect. I am building a project in <Language/Framework>.
Based on the agy-suite declarative schemas, generate:
1. project.toml: with gates for compilation, unit tests, and linting.
2. tools.toml: declaring test runners and diagnostic tools with enforce_sandbox = true,
   strict regex parameter validation, and enums for mutually exclusive flags.
3. hooks.toml: with a 2-strike compiler circuit breaker.
4. agents/: 2 specialized agent personas with clean allowed_dirs and mcp_tools allowlists."
```

The AI outputs pure TOML and Markdown. Drop them into `.agy/`, run `agy-ctrl`, and your bespoke air-gapped engineering team is live in seconds. **Zero code written. Zero compilation needed.**

---

## 👥 The Flagship 10-Agent Reference Fleet

Included out-of-the-box in `plugins/agy-suite/` is our flagship reference fleet, built under the Silicon Design Verification (DV) discipline—where implementation engineers never author their own testbenches:

| Agent Profile | Role & Specialty | Permitted Directory Scope | Content & Security Policy |
| :--- | :--- | :--- | :--- |
| **[`coder`](plugins/agy-suite/agents/coder.md)** | Production systems implementer (Rust, RISC-V asm) | `src/`, `crates/`, `boards/`, `targets/` | Full code mutation; `Cargo.toml` protected by `manifest_guard` |
| **[`test-author`](plugins/agy-suite/agents/test-author.md)** | Design Verification (DV) & Formal Test Engineer | `tests/`, `benches/`, `src/` | `tests_only` in `src/` (`mod tests`); formal proofs (Kani, Loom, Miri) |
| **[`doc-author`](plugins/agy-suite/agents/doc-author.md)** | Specification & Rustdoc Author | `docs/`, `specs/`, `README.md`, `ARCHITECTURE.md` | `comments_only` in `src/` (in-place `///` rustdoc without logic edits) |
| **[`planner`](plugins/agy-suite/agents/planner.md)** | Architecture & Verification Strategy Rubber-Duck | `docs/`, `ARCHITECTURE.md` | Authoritative architecture RFCs & upfront verification planning |
| **[`copilot`](plugins/agy-suite/agents/copilot.md)** | Chief of Staff & Workflow Orchestrator | Read-only across repository | Read-only code inspection; Jujutsu workspace rebase/merge |
| **[`finisher`](plugins/agy-suite/agents/finisher.md)** | Adversarial Release Gatekeeper | Read-only across repository | Pre-release gauntlet scorecard (fmt, clippy, tests, diffs) |
| **[`spec-checker`](plugins/agy-suite/agents/spec-checker.md)** | Spec-vs-Code & ISA Compliance Auditor | Read-only across repository | AST queries, opcode audits, and spec-to-source traceability |
| **[`perf-analyst`](plugins/agy-suite/agents/perf-analyst.md)** | Hardware Profiler & Benchmark Engineer | `benches/`, `docs/`, `.agy/artifacts/` | Microbenchmarking (Divan/Criterion), flamegraphs, `llvm-mca` |
| **[`mini`](plugins/agy-suite/agents/mini.md)** | Symbolic Math & Calculation Specialist | Strictly air-gapped | Network-less Bubblewrap container using Python/SymPy |
| **[`sysadmin`](plugins/agy-suite/agents/sysadmin.md)** | Arch Linux / CachyOS System Administrator | Full system | Supervised shell; interactive operator confirmation on mutating actions |

---

## 🛠️ Codeless Extensibility

You do not need to write Rust code or recompile binaries to customize tools or agents:

### 1. Add a Tool in `tools.toml` (30 Seconds)
Declare tools in [`plugins/agy-suite/tools.toml`](plugins/agy-suite/tools.toml) or `.agy/tools.toml`:
```toml
[tools.my_scanner]
inherit = "base_tools.cargo_base"
description = "Executes custom security scan with regex-validated arguments"
action = "parameterized_command"
command = ["cargo", "audit"]
enforce_sandbox = true                          # Run in isolated Bubblewrap jail
mutating = false
args = [
    { name = "target", required = false, flag = "--target", regex = "^[a-zA-Z0-9_-]+$", description = "Crate target name" },
    { name = "mode", required = false, default = "advisories", enum = ["advisories", "bans", "licenses"], description = "Audit check mode" }
]
```

### 2. Define a Specialized Agent
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
Once saved, `my-agent` immediately surfaces in `agy-ctrl` with full gate enforcement!

---

## 📚 Deep Dives & Documentation

For in-depth systems design, formal verification contracts, and extended guides:

* 🏛️ **[Canonical System Architecture (`ARCHITECTURE.md`)](ARCHITECTURE.md)**:
  * Non-negotiable system invariants & error handling requirements.
  * The Three Declarative Contracts (`tools.toml`, `hooks.toml`, `project.toml`).
  * Process tree supervision & hang immunity (`SubprocessSupervisor`).
  * Bubblewrap container sandboxing & private loopback network model.
  * The Silicon Design Verification (DV) methodology.
* 📖 **[The agy-suite Engine Manual (Online Docs)](https://jeronimocapelle.github.io/agy-suite/)**:
  * [Pre-Built Ecosystem Starter Kits](https://jeronimocapelle.github.io/agy-suite/templates/overview.html)
  * Step-by-step setup and real-world workflow tutorials.
  * Deep dives into the 4-phase verification sandwich.
  * Sub-millisecond latency & memory footprint benchmarks.

---

## ⚖️ License

Licensed under the **GNU Affero General Public License v3.0 or later** ([AGPL-3.0-or-later](LICENSE)).
