# agy-suite Tooling & Agent Capability Knowledge Matrix

This rule provides all agents in the `agy-suite` environment with ambient awareness of the local toolbelt, security boundaries, and specialized agent assignments from `cargo_tooling_audit_and_assignment.md`.

---

## 1. The 3-Tier Tooling Security Architecture

In a **zero-ambient-authority, default-deny** systems programming environment:

1. **Category A: Strictly USER-ONLY (Hard Block for Autonomous Agents)**
   - **Credentials & Publishing:** `cargo login`, `cargo logout`, `cargo owner`, `cargo package`, `cargo publish`, `cargo yank`, `credential-oo7`. Never leak or manipulate credentials.
   - **Host Package Mutation:** `cargo install`, `cargo uninstall`, `cargo binstall`, `cargo install-update`. Reserved for manual operator use or `sysadmin` with explicit user confirmation.
   - **Destructive Build Wipers:** `cargo clean`, `cargo clean-all`, `cargo sweep`. Wipes compilation caches across the disk; strictly operator-only.
   - **Arbitrary Binary Runner:** `cargo run`. Bypasses test sandboxes; user-only.

2. **Category B: Gated Operator / Interactive Planner (Human Confirmation Required)**
   - **Dependency Mutation:** `cargo add`, `cargo remove`, `cargo update`, `cargo vendor`.
   - **Workspace Scaffolding:** `cargo new`, `cargo init`, `cargo generate` (cargo-generate templates).
   - **Lockfile & Network:** `cargo fetch`, `cargo generate-lockfile`.
   - **Tree Mutation & Auto-fixes:** `cargo fmt`, `cargo fix`.
   - **Cross-Compilation:** `cargo zigbuild` (cross-compiles targeting musl, windows, aarch64, riscv without Docker).

3. **Category C, D, E: Bounded Agent Capabilities via MCP Guards (`tools.toml`)**
   - Granted through declarative `tools.toml` guards: strict argv array execution, zero shell injection, output truncation, and token-frugal capture modes.

---

## 2. Specialized Agent Capability Directory

When navigating tasks, triaging user requests, or auditing code, agents must route to or recommend the right tool and specialist:

| Domain | Specialized Agent | Toolbelt & Capabilities | When to Recommend / Dispatch |
| :--- | :--- | :--- | :--- |
| **Workflow Coordination** | `copilot` | `read_project_file`, `write_file`, `modify_code`, `search_codebase`, `inspect_module_structure`, `run_quick_check`, `query_tool_help`, `invoke_subagent` | Chief of Staff. Triages tasks, answers questions, handles single-file fixes, or delegates multi-file work to specialists. |
| **Architecture & Inception** | `planner` | `read_project_file`, `write_file`, `modify_code`, `inspect_module_structure` (`cargo-modules`), `query_project_spec` | Co-designs module boundaries, trait contracts, and invariants in `ARCHITECTURE.md`. |
| **Production Implementation** | `coder` | `read_project_file`, `modify_code`, `write_file`, `run_quick_check`, `run_cargo_verification`, `run_cargo_test`, `run_cargo_clippy`, `run_cargo_fmt` | Autonomous implementer. Confined to `src/` by per-agent guards. Iterates against failing tests until green. |
| **Test Suite Engineering** | `test-author` | `cargo-nextest`, `cargo-llvm-cov`, `cargo-mutants`, `cargo-insta`, `cargo-fuzz`, `run_property_tests`, `run_concurrency_tests` | Category C. Writes unit/integration suites in `tests/`, verifies branch coverage, runs mutation testing to find weak assertions. |
| **Performance & Systems Profiling** | `perf-analyst` | `cargo-show-asm`, `cargo-bench`, `cargo-careful`, `cargo-miri`, `cargo-flamegraph`, `cargo-perf`, `cargo-bloat`, `hyperfine`, `samply`, binutils (`size`, `nm`, `objdump`) | Category D. Disassembles symbols to assembly/LLVM-IR, audits UB/memory safety, profiles CPU hotspots, analyzes binary footprint. |
| **Static Analysis & SemVer** | `spec-checker` | `cargo-clippy`, `cargo-deny`, `cargo-machete`, `cargo-semver-checks`, `cargo-public-api`, `cargo-hack`, `cargo-modules`, `cargo-expand`, `ast-grep` | Category E. Audits RustSec security advisories, unused dependencies, breaking public API changes, and feature matrix flags. |
| **Release Sentinel & Gatekeeper** | `finisher` | `inspect_workspace_diff`, `inspect_parent_commit`, `inspect_workspace_conflicts`, `run_cargo_nextest`, `run_cargo_clippy`, `run_cargo_doc`, `run_cargo_fmt`, `run_cargo_check_targets`, `run_git_cliff` | Final gatekeeper. Audits diffs against `ARCHITECTURE.md`, enforces passing pre-commit gauntlet, generates changelogs, suggests deep checks. |
| **Host System Administration** | `sysadmin` | `view_file`, `grep_search`, `run_command`, `ask_question`, `query_tool_help` | Arch Linux administration (`pacman`, `paru`, `systemctl`, `journalctl`, kernel, storage, networking) with interactive human confirmation. |
| **Symbolic Math & Solver** | `mini` | `eval_symbolic_math` | Low-token subagent for complex arithmetic, symbolic calculus, and data digestion in Bubblewrap sandbox. |

---

## 3. High-Signal Verification Recommendations (Cross-Tool Matrix)

When recommending verification actions to the operator or in an audit report:

* **Unsafe Code, Raw Pointers, or FFI Modified:** Recommend `cargo careful` (fast UB runtime check) and `cargo miri` (deep pointer provenance and stacked/tree borrows interpreter).
* **Concurrency, Channels, or Atomics Modified:** Recommend `cargo loom` (concurrency permutation checking) and `run_concurrency_tests`.
* **State Machines, Enums, or Arithmetic Boundaries:** Recommend `cargo kani` (SMT model checking) or `cargo-fuzz` (libFuzzer panic discovery).
* **Dependency Health & Supply Chain:** Recommend `cargo deny check` (RUSTSEC advisories and copyleft license check) and `cargo machete` (unused dependency removal).
* **Public API & Versioning Changes:** Recommend `cargo semver-checks check-release` and `cargo public-api diff` to prevent accidental semver violations.
* **Feature Flag Permutations:** Recommend `cargo hack check --each-feature` to prevent compilation failures under `--no-default-features`.
* **Binary Size & Monomorphization:** Recommend `cargo bloat --release -n 30` and `cargo size` to locate bloated generic instances.
* **CLI Benchmark Regressions:** Recommend `hyperfine` with warmup runs for statistically rigorous timing comparisons.
* **Release Changelogs:** Recommend `run_git_cliff` to parse conventional commits into clean release notes.

---

## 4. Live Ground-Truth Documentation via `query_tool_help`

Never guess or hallucinate CLI tool flags or subcommands based on pre-training memory (which drifts on modern tools like `jj` or cargo plugins). 
Agents should invoke `query_tool_help(tool_name = "...")` to retrieve live ground-truth syntax from local `navi` recipes, `tldr` cheatsheets, native `--help`, or system `man` pages. Calling `query_tool_help` with no arguments returns the active workspace tooling directory.
