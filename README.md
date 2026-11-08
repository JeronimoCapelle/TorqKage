# agy-suite

[![CI](https://github.com/JeronimoCapelle/agy-suite/actions/workflows/ci.yml/badge.svg?branch=main)](https://github.com/JeronimoCapelle/agy-suite/actions/workflows/ci.yml)
[![Rust Edition](https://img.shields.io/badge/rust-2024%20edition-orange.svg)](https://www.rust-lang.org/)
[![License: AGPL-3.0-or-later](https://img.shields.io/badge/License-AGPL--3.0--or--later-blue.svg)](LICENSE)
[![Documentation](https://img.shields.io/badge/docs-manual-blue.svg)](https://jeronimocapelle.github.io/agy-suite/)
[![MCP Protocol](https://img.shields.io/badge/MCP-2024--11--05-green.svg)](https://modelcontextprotocol.io/)
[![Ratatui](https://img.shields.io/badge/Ratatui-0.30-purple.svg)](https://ratatui.rs/)

**Enterprise-grade capability sandbox, declarative MCP daemon, and Ratatui 0.30 interactive flight deck for Google Antigravity.**

<p align="center">
  <img src="assets/demo.gif" alt="agy-suite Interactive Terminal Control Deck Demo" width="100%" />
</p>

---

## 🎯 Why agy-suite?

Autonomous AI coding agents are extraordinarily capable, but unleashing them with raw terminal access in production repositories creates critical risks: non-deterministic execution, destructive shell accidents, massive token bills, and dirty git trees.

`agy-suite` hardens [Google Antigravity (`agy`)](https://github.com/google/antigravity) into a disciplined, capability-based systems engineering platform written in 100% Safe Rust (`#![forbid(unsafe_code)]`):

| The Raw AI Coding Problem | The `agy-suite` Solution |
| :--- | :--- |
| **Ambient Shell Poisoning:** Agents execute uncontrolled `bash` commands with host authority, risking accidental `rm -rf`, network exfiltration, or credential leaks. | 🛡️ **Container-Grade Bubblewrap Sandboxing:** Precision-jail runs with private PID/IPC/network namespaces, private loopback only, zero external sockets, and read-only host mounts. |
| **Token Exhaustion:** Inlining multi-kilobyte prompt instructions, markdown skills, and manuals burns 2,000–5,000 baseline tokens on *every single turn*. | ⚡ **Compiled Native Interceptors (<1ms, <500KB RSS):** Bounded Rust daemons handle tool dispatch, parameter validation, and output slicing offline with zero prompt token waste. |
| **Lost Conversational State:** Terminal drops or shell disconnects lose the active conversational thread, forcing you to re-prompt from scratch. | 🔄 **Multi-Source Session Resumption:** Embedded SQLite & history correlation automatically surface resumable sessions directly in `agy-ctrl` (`[Enter]` / `r`). |
| **Worktree & Buffer Pollution:** Unsupervised model edits pollute your active working copy and editor buffers with uncommitted churn. | 🌿 **Jujutsu Workspace Isolation:** Agents operate in dedicated, sparse Jujutsu working copies (`.agy/workspaces/<agent>`) without ever mutating your active `@` revision. |
| **Runaway Failure Loops:** Agents hallucinate or repeat the same failing compiler fix across endless expensive API turns. | 🛑 **2-Strike Circuit Breaker:** Automatically trips after 2 consecutive compiler/test failures, halting execution until human review or single-key reset (`c`). |

---

## ⚡ Quickstart

### 1. Instant Install (Precompiled Binaries)
Install the full suite and register the Antigravity plugin with a single command (downloads precompiled native binaries for your architecture; zero Rust compilation required):

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
You can also invoke specialized agents directly from the command line:

```bash
# Production systems implementation in src/ against failing tests
agy --agent coder

# Design Verification (DV) engineer authoring adversarial edge-case tests
agy --agent test-author

# Technical documentation, rustdoc, and architectural RFC author
agy --agent doc-author

# Adversarial release gatekeeper running pre-release gauntlets
agy --agent finisher
```

### 3. Sub-Second Build Acceleration
`agy-suite` defaults to 100% standard stable Rust out-of-the-box. If your system has Cranelift, Mold, or `sccache` installed, use the included accelerator:

```bash
# Auto-detects Cranelift + Mold + sccache for sub-second compile cycles:
./plugins/agy-suite/bin/cargo-fast build
./plugins/agy-suite/bin/cargo-fast test
```

---

## 👥 The 10-Agent Specialized Fleet

Rather than relying on a single monolithic prompt, `agy-suite` deploys a 10-persona fleet operating under the Silicon Design Verification (DV) discipline—where implementation engineers never author their own verification harnesses:

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
Declare tools in [`plugins/agy-suite/tools.toml`](plugins/agy-suite/tools.toml):
```toml
[tools.my_scanner]
inherit = "base_tools.cargo_base"
description = "Executes custom security scan with regex-validated arguments"
action = "parameterized_command"
command = ["cargo", "audit"]
enforce_sandbox = true                          # Run in isolated Bubblewrap jail
mutating = false

[[tools.my_scanner.args]]
name = "target"
required = false
flag = "--target"
regex = "^[a-zA-Z0-9_-]+$"                      # Hard input validation
```

### 2. Define a Specialized Agent
Create [`plugins/agy-suite/agents/my-agent.md`](plugins/agy-suite/agents/):
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
  * Step-by-step setup and real-world workflow tutorials.
  * Deep dives into the 4-phase verification sandwich.
  * Sub-millisecond latency & memory footprint benchmarks.

---

## ⚖️ License

Licensed under the **GNU Affero General Public License v3.0 or later** ([AGPL-3.0-or-later](LICENSE)).
