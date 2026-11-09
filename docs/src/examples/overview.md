# Real-World Workflows & Examples

This section walks through hands-on examples of applying `Torqkage` to real-world software engineering workflows.

---

## Example Guides

1. **[End-to-End Compiler Refactor](compiler-refactor.md)**  
   How to chain `planner` → `test-author` → `coder` → `finisher` to implement a breaking architectural refactor with zero regressions.

2. **[Adding a Custom Declarative Tool](custom-mcp-tool.md)**  
   Step-by-step guide to adding a new tool to `tools.toml` and granting it to an agent with custom Bubblewrap isolation.

3. **[Isolated Subagent Worktrees](subagent-worktrees.md)**  
   How `copilot` and `coder` spin up isolated Jujutsu worktrees (`.tkg/workspaces/<agent>`) to modify code safely without polluting the main working copy `@`.
