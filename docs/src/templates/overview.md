# Pre-Built Ecosystem Starter Kits

`Torqkage` is a **declarative governance engine and sandboxed MCP runtime**. Rather than locking you into a single pre-configured fleet, the engine enables developers—and autonomous AIs—to define completely custom, air-gapped agent teams using pure TOML and Markdown.

To accelerate bootstrapping, the engine includes ready-to-use, production-hardened starter kits located in `docs/templates/`:

| Starter Kit | Target Ecosystem | Included Tooling & Sandboxing Highlights | Agent Team |
| :--- | :--- | :--- | :--- |
| **[Go Microservices](go-setup.md)** | Go 1.22+ (`cmd/`, `internal/`) | `go test` in Bubblewrap sandbox, `enum` mode flags (`-short`, `-race`), database migration permission gates | `coder`, `reviewer` |
| **[Python & Data Engineering](python-setup.md)** | Python 3.12+ (uv / pytest / ruff / mypy) | Sandboxed `pytest` with keyword regex filters, Mypy static typing, automated Ruff fixer | `coder`, `tester` |
| **[TypeScript & Fullstack Web](typescript-setup.md)** | Node.js / React / Next.js (pnpm / Vitest / tsc) | Sandboxed `vitest` with target scoping, `tsc --noEmit` gate, ESLint zero-warning checks | `frontend`, `reviewer` |
| **[Standard Rust Systems](rust-setup.md)** | Rust 2024 Edition (Cargo workspaces) | Bubblewrap sandboxed `cargo test`, `enum` targets (`--lib`, `--bins`), Clippy `-D warnings` gate | `coder`, `doc-author` |
| **[Adversarial QA Auditor](qa-tester.md)** | Ecosystem-agnostic | Fuzz testing with LLVM sanitizers (`enum`), schema verification, unrestricted adversarial probing | `tester` (`tester.md`) |

---

## ⚡ How to Deploy a Starter Kit in 30 Seconds

1. Copy the desired starter kit into your workspace's `.tkg/` directory:
   ```bash
   mkdir -p .tkg
   cp -r /path/to/TorqKage/docs/templates/go/* .tkg/
   ```

2. Boot the interactive flight deck:
   ```bash
   tkg
   ```
   The control deck automatically detects the declarative contracts:
   - `project.toml`: Evaluates pre-launch quality gates and registers build cache bind mounts.
   - `tools.toml`: Generates MCP tool definitions with regex-validated schemas and container isolation.
   - `hooks.toml`: Activates the 2-strike compiler circuit breaker and operator tool gates.
   - `agents/*.md`: Populates the Fleet Matrix with custom agent boundaries.

---

## 🤖 The "Prompt-to-Fleet" Workflow (LLM as Fleet Architect)

Because `Torqkage` configurations are 100% declarative, you can prompt any LLM (Claude, ChatGPT, Gemini, or Antigravity) to design a bespoke fleet for your exact tech stack:

```text
"Act as an Torqkage Fleet Architect. I am building a project in <Language/Framework>.
Based on the Torqkage declarative schemas, generate:
1. project.toml: with gates for compilation, unit tests, and linting.
2. tools.toml: declaring test runners and diagnostic tools with enforce_sandbox = true,
   strict regex parameter validation, and enums for mutually exclusive flags.
3. hooks.toml: with a 2-strike compiler circuit breaker.
4. agents/: 2 specialized agent personas with clean allowed_dirs and mcp_tools allowlists."
```
