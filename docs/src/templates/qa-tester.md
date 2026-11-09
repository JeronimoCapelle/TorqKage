# Adversarial QA Auditor & Fuzzer (`tester.md`)

The QA Auditor & Fuzzer setup (`docs/templates/qa-tester/`) provides an adversarial testing agent designed to systematically audit tools, uncover CLI argument injection vulnerabilities, test boundary traversal, and detect runtime schema mismatches.

## Directory Structure
```
.tkg/
├── project.toml       # Tooling sanity gate
├── tools.toml         # Sandboxed fuzzing (`cargo fuzz`) and schema validator
├── hooks.toml         # Circuit breaker and tool policy
└── agents/
    └── tester.md      # Adversarial QA auditor agent with mcp_tools: ["*"]
```

---

## 1. Role of `tester.md`
`tester` has full access to the complete tool suite (`mcp_tools: ["*"]`) with a mandate to actively probe:
1. **Flag Injections:** Passing hyphen-prefixed strings (`--help`, `-O3`, `-D warnings`) into free-text parameters to verify argument escaping.
2. **Path Traversals:** Probing boundary guards with `../` and absolute paths to verify workspace confinement.
3. **Conflicting Arguments:** Passing mutually exclusive options to test declarative `conflicts_with` handling.
4. **Output Hygiene:** Verifying that ANSI escape codes are stripped and large payloads are properly truncated.

---

## 2. Sandboxed Fuzzing with LLVM Sanitizers
In `tools.toml`, `run_fuzz_target` leverages Bubblewrap isolation and LLVM sanitizer enums:

```toml
[tools.run_fuzz_target]
description = "Executes fuzz target binary inside sandboxed Bubblewrap container with time bounds"
action = "parameterized_command"
category = "testing"
enforce_sandbox = true
command = ["cargo", "fuzz", "run"]
args = [
    { name = "target", required = true, regex = "^[a-zA-Z0-9_]+$", description = "Fuzz target name" },
    { name = "sanitizer", required = false, default = "address", enum = ["address", "memory", "thread", "undefined"], flag = "-s", description = "LLVM sanitizer to attach" },
    { name = "max_time", required = false, default = "30", flag = "-max_total_time", regex = "^[0-9]+$", description = "Timeout in seconds" }
]
output_capture = "stderr"
truncate_lines = 100
timeout_seconds = 60
```

---

## 3. Findings Reporting Standard
When `tester` finds an inconsistency or error, it outputs a structured vulnerability dossier:

```markdown
### Detailed Vulnerability Dossier
- **Tool Name:** `tool_name`
- **Reproducible Call:** `{ "tool": "tool_name", "arguments": { ... } }`
- **Observed Behavior:** Stderr or unhandled exception.
- **Expected Behavior:** Clean parameter validation error.
- **Recommended Fix:** Missing `--` delimiter or regex adjustment in `tools.toml`.
```
