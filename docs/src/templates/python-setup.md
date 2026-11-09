# Python & Data Engineering Starter Kit

The Python starter kit (`docs/templates/python/`) targets modern Python 3.12+ applications managed with `uv`, `pytest`, `mypy`, and `ruff`.

## Directory Structure
```
.tkg/
├── project.toml       # Type checking (mypy), tests (pytest), and formatting (ruff)
├── tools.toml         # Sandboxed `pytest_run`, static `mypy_check`, and mutating `ruff_fix`
├── hooks.toml         # 2-strike compiler circuit breaker and subagent admission
└── agents/
    ├── coder.md       # Application logic author scoped strictly to src/
    └── tester.md      # Test engineer scoped strictly to tests/
```

---

## 1. `tools.toml` Highlights
Demonstrates Bubblewrap sandboxing with boolean flags and directory enums:

```toml
[tools.pytest_run]
description = "Executes pytest inside an isolated Bubblewrap sandbox with keyword filtering"
action = "parameterized_command"
category = "testing"
enforce_sandbox = true
command = ["uv", "run", "pytest", "-q"]
args = [
    { name = "filter", required = false, flag = "-k", regex = "^[a-zA-Z0-9_]+$", description = "Test name keyword filter" },
    { name = "scope", required = false, default = "tests", enum = ["tests", "tests/unit", "tests/integration"], description = "Directory scope to execute" },
    { name = "fail_fast", required = false, default = "false", flag = "-x", type = "boolean", description = "Exit instantly on first failure" }
]
output_capture = "stdout"
truncate_lines = 100
timeout_seconds = 60

[tools.ruff_fix]
description = "Applies automated Ruff linter fixes (mutating)"
action = "static_command"
category = "formatting"
mutating = true
require_permission_gate = false
command = ["uv", "run", "ruff", "check", "--fix", "."]
output_capture = "stdout"
truncate_lines = 40
timeout_seconds = 20
```

---

## 2. Silicon DV Split: `coder` vs `tester`

Under the Silicon DV discipline, the engineer writing business logic never writes their own testbench:

- **`coder.md`:** Write access allowed strictly to `src/`; tests in `tests/` are disallowed to prevent tampering with assertions.
- **`tester.md`:** Write access allowed strictly to `tests/`; application code in `src/` is disallowed.
