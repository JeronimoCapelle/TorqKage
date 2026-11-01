# agy-suite

High-performance, least-privilege systems Rust project initialized from `cgen`.

## 🛠️ Developer & Tooling Workflow

This skeleton is pre-configured with the complete suite of low-level verification and profiling tools:

### 1. Build & Run
- **Main Binary:** `cargo run` (or `cargo r`)
- **Auxiliary Binary:** `cargo run --bin bin1`
- **Example:** `cargo run --example basic`
- **Fast Dev Build:** `cargo build --profile fast` (uses Cranelift backend)

### 2. Testing & Verification
- **Nextest Test Runner:** `cargo t` (or `cargo nextest run`)
- **Snapshot Testing:** `cargo insta` (runs snapshot review/accept via `cargo-insta`)
- **Property Testing:** `cargo test --test property_test` (generative testing via `proptest`)
- **Concurrency Permutation Testing:** `cargo test --test concurrency_test` (exhaustive atomics verification via `loom`)
- **Mutation Testing:** `cargo mutants` (verifies assertion quality with `cargo-mutants`)
- **Feature Matrix Check:** `cargo hack` (checks `--each-feature` via `cargo-hack`)
- **Memory & UB Checks:** `cargo careful` or `cargo t-miri`

### 3. Coverage & Profiling
- **HTML Coverage:** `cargo cov` (opens browser coverage report via `cargo-llvm-cov`)
- **Text Coverage Summary:** `cargo cov-summary`
- **Allocation Benchmarking:** `cargo bench` (microbenchmarks & heap tracking via `divan`)
- **Deterministic Hardware & Heap Benchmarking:** `cargo bench --bench gungraun_bench` (exact instructions, cache misses, and heap tracking via `gungraun` / Valgrind DHAT)
- **Statistical Benchmarking:** `cargo bench --bench criterion_bench` (regression & confidence intervals via `criterion2`)
- **CPU Flamegraph:** `cargo fg` (generates SVG flamegraph via `flamegraph`)
- **Sampling Flamechart:** `cargo samply-rec` (records profile for Firefox Profiler)

### 4. Hygiene & Auditing
- **Full Policy & Advisory Audit:** `cargo deny-all` (RustSec advisories, licenses, bans, sources via `cargo-deny`)
- **Unused Dependencies:** `cargo unused` (scans via `cargo-machete`)
- **Public API Diff:** `cargo public-api` (diffs exposed API surface)
- **Module Hierarchy:** `cargo modules structure`
- **Assembly Disassembly:** `cargo asm-fn`

---

## 📋 Technical Debt & Future Hardening Backlog

1. **Re-Enable `cargo::unused_dependencies` Lint:**
   - Temporarily set to `"allow"` in `[workspace.lints.cargo]` during workspace bootstrapping.
   - **Action:** Re-enable strict checking (`"warn"` / `"-D warnings"`) once all crate features in `agy-ctrl` and `agy-mcp` are fully wired up.

2. **Refactor Boolean Security Flags to Typed Enums:**
   - `ToolDefinition` in `agy-mcp` currently uses boolean flags matching `tools.toml` toggles (`enforce_workspace_bounds`, `ban_dangerous_files`, etc.), requiring `#[allow(clippy::struct_excessive_bools)]`.
   - **Action:** Refactor these primitive flags into typed two-variant or multi-variant domain enums (`BoundaryPolicy`, `SanitizationPolicy`, etc.) to prevent boolean blindness and eliminate the lint suppression.

3. **Evaluate Safe Crate Abstractions for Process Hygiene:**
   - `SubprocessSupervisor` in `agy-common` currently uses scoped `unsafe` blocks for `libc::prctl(PR_SET_PDEATHSIG)` and `libc::setpgid`.
   - **Action:** Audit established community crates (e.g. `process-wrap`, `command-group`, or safe bindings) that encapsulate Linux orphan prevention in safe abstractions, allowing `unsafe_code = "forbid"` to be restored at the workspace root.
