# TypeScript & Fullstack Web Starter Kit

The TypeScript starter kit (`docs/templates/typescript/`) targets Node.js, React, or Next.js codebases managed with `pnpm`, `Vitest`, and TypeScript.

## Directory Structure
```
.agy/
├── project.toml       # tsc --noEmit, vitest, and eslint quality gates
├── tools.toml         # Sandboxed `vitest_run`, static `tsc_check`, and `eslint_check`
├── hooks.toml         # Circuit breaker and tool policy
└── agents/
    ├── frontend.md    # Component author scoped to apps/web and packages/ui
    └── reviewer.md    # Read-only audit persona checking diffs and bundle impact
```

---

## 1. `tools.toml` Highlights
Demonstrates argument execution modes (`run` vs `related`) with path regex:

```toml
[tools.vitest_run]
description = "Executes Vitest in a sandboxed Bubblewrap container with target scoping"
action = "parameterized_command"
category = "testing"
enforce_sandbox = true
command = ["pnpm", "vitest"]
args = [
    { name = "mode", required = false, default = "run", enum = ["run", "related"], description = "Execution mode: run all or related only" },
    { name = "target", required = false, regex = "^[a-zA-Z0-9_/.-]+$", description = "Test file or spec path" }
]
output_capture = "stdout"
truncate_lines = 100
timeout_seconds = 45
```

---

## 2. Blast Boundary Protection
The `frontend.md` agent is explicitly disallowed from modifying backend database configurations:

```yaml
allowed_dirs:
  - apps/web
  - packages/ui
disallowed_dirs:
  - packages/database
  - tests/e2e
```
