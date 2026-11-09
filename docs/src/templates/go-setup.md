# Go Microservices Starter Kit

The Go Microservices starter kit (`docs/templates/go/`) configures an air-gapped engineering environment for production Go services.

## Directory Structure
```
.tkg/
├── project.toml       # Compilation, test, formatting, and linting gates
├── tools.toml         # Sandboxed `go test`, static `go build`, and guarded `migrate`
├── hooks.toml         # 2-strike compiler circuit breaker and mutating policy
└── agents/
    ├── coder.md       # Implementation agent scoped to cmd/ and internal/
    └── reviewer.md    # Read-only adversarial reviewer auditing diffs
```

---

## 1. `project.toml`
Declares quality gates and binds the Go build cache into sandbox instances:

```toml
[project]
name = "my-go-service"
language = "go"
version = "0.1.0"

[workspace]
containers = ["cmd", "internal", "pkg"]
ignored_paths = [".cache/go-build", ".tkg/state", ".tkg/workspaces"]

[sandbox]
bind_mounts = [".cache/go-build"]

[gates.compilation]
command = ["go", "build", "./..."]
timeout_seconds = 30
failure_message = "Go build failed (`go build ./...`)."

[gates.tests]
command = ["go", "test", "./..."]
timeout_seconds = 45
failure_message = "Go test suite execution failed (`go test ./...`)."
```

---

## 2. `tools.toml`
Demonstrates Bubblewrap container sandboxing, enum argument flags, and operator gates:

```toml
[tools.go_test]
description = "Executes Go test runner in a sandboxed Bubblewrap container with package selection"
action = "parameterized_command"
category = "testing"
enforce_sandbox = true
command = ["go", "test"]
args = [
    { name = "package", required = false, default = "./...", regex = "^[a-zA-Z0-9_/.-]+$", description = "Package path or pattern" },
    { name = "mode", required = false, default = "short", enum = ["short", "race", "cover"], flag = "-", description = "Execution mode: -short, -race, or -cover" },
    { name = "run", required = false, flag = "-run", regex = "^[a-zA-Z0-9_]+$", description = "Specific test function regex to execute" }
]
output_capture = "stdout"
truncate_lines = 100
timeout_seconds = 60

[tools.go_migrate]
description = "Runs database schema migrations (requires operator permission gate)"
action = "parameterized_command"
category = "database"
mutating = true
require_permission_gate = true
command = ["migrate", "-path", "migrations", "-database", "postgres://localhost/app"]
args = [
    { name = "action", required = true, enum = ["up", "down", "version", "force"], description = "Migration action to execute" },
    { name = "steps", required = false, default = "1", regex = "^[0-9]+$", description = "Number of migration steps" }
]
output_capture = "stdout"
truncate_lines = 40
timeout_seconds = 30
```

---

## 3. Agents Frontmatter

### `coder.md` (Implementation)
```yaml
---
name: coder
effort: medium
permissionMode: acceptEdits
pre_launch_checks:
  - compilation
  - vcs_conflicts
allowed_dirs:
  - cmd
  - internal
  - pkg
disallowed_dirs:
  - migrations
  - tests
mcp_tools:
  - read_project_file
  - write_file
  - mpatch_edit
  - go_test
  - go_build
---
```

### `reviewer.md` (Read-Only Gatekeeper)
```yaml
---
name: reviewer
effort: high
permissionMode: plan
pre_launch_checks:
  - compilation
  - tests
  - vcs_conflicts
disallowed_dirs:
  - "*"
mcp_tools:
  - read_project_file
  - inspect_workspace_diff
  - go_test
---
```
