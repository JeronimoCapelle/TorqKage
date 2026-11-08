# Contributing to `agy-suite`

Thank you for your interest in contributing to `agy-suite`! This project provides high-assurance systems tooling, declarative MCP daemons, and terminal orchestrators for Google Antigravity.

---

## 🛡️ Core Architectural Invariants

Before writing code, please review [`ARCHITECTURE.md`](../ARCHITECTURE.md). All contributions must uphold these hard requirements:

1. **Pure Safe Rust:** Every crate enforces `#![forbid(unsafe_code)]`. No `unsafe` blocks are accepted.
2. **Deterministic Sandboxing:** Tools and subprocesses run within non-networked Bubblewrap (`bwrap`) containers. Never bypass path isolation or file boundaries.
3. **Hermetic & Machine-Agnostic:** Code and tests must never assume fixed local host paths (such as `/home/<user>`) or machine-specific environments. All tests must run cleanly in CI containers.
4. **Zero Token Waste:** Agent prompts, manifests, and errors must remain concise, structured, and free of opinionated hardcoded assumptions.
5. **No Opinionated Language Hardcoding:** Framework tools and error handlers must remain strictly language- and context-agnostic.

---

## 🛠️ Development Setup & Fast Iteration

We optimize compilation using Cranelift and the Mold linker:

```bash
# 1. Clone the repository
git clone https://github.com/JeronimoCapelle/agy-suite.git
cd agy-suite

# 2. Build with Cranelift + Mold (<6 seconds)
./install.sh --fast

# 3. Launch the offline terminal control deck
agy-ctrl
```

### Pre-Submission Verification Gauntlet

Before opening a pull request, ensure your branch passes all quality gates locally:

```bash
# 1. Unit & Integration Tests (100% passing)
cargo test --all-targets

# 2. Strict Pedantic Clippy
cargo clippy --all-targets -- -D warnings

# 3. Dependency & Supply Chain Verification
cargo deny check
```

---

## 📝 Conventional Commits

We follow the [Conventional Commits](https://www.conventionalcommits.org/) specification to automate releases and changelogs:

* `feat(crate): description` – New capability or agent tool
* `fix(crate): description` – Bug fix or boundary correction
* `perf(crate): description` – Performance optimization or allocation reduction
* `refactor(crate): description` – Code simplification with no behavioral changes
* `test(crate): description` – Unit test or integration test additions
* `docs: description` – Documentation or specification updates

---

## 🔀 Pull Request Process

1. Fork the repo and create your branch from `main`: `git checkout -b feat/my-feature`.
2. Ensure all 3 verification checks pass (`cargo test`, `cargo clippy`, `cargo deny check`).
3. Complete the pull request template with a concise summary and verification log.
4. Maintainers will review your submission promptly. Thank you for making systems programming safer!
