# Example: Isolated Subagent Worktrees

One of the unique advantages of `agy-suite` is **VCS-level working copy isolation** powered by [Jujutsu (`jj`)](https://github.com/martinvonz/jj).

---

## The Problem: Editor Buffer Pollution

When an autonomous AI agent modifies code in your main working copy:
1. Files in your IDE (VS Code, Neovim, Zed) suddenly reload or change out from under you.
2. Unfinished or broken edits disrupt your active terminal checks.
3. If the agent makes a mistake, reverting leaves dangling files and untracked artifacts.

---

## The Solution: Automatic Agent Worktrees

`agy-suite` solves this via `agy_common::JujutsuWorkspace`. When an agent is launched with `--isolated` or by toggling <kbd>w</kbd> in `agy-ctrl`:

```
repo_root/
├── src/                    <-- Your working copy (@). Totally clean and unpolluted.
├── Cargo.toml
└── .agy/
    └── workspaces/
        └── coder/          <-- Isolated jj workspace (coder@).
            ├── src/        <-- Agent implements, edits, and compiles here!
            └── Cargo.toml
```

### Lifecycle

1. **Instantiation:** `agy-ctrl` creates `.agy/workspaces/coder` linked to a dedicated commit branch (`coder@`).
2. **Autonomous Execution:** `coder` compiles, tests, and edits strictly inside `.agy/workspaces/coder`. Your main editor buffers never move.
3. **Inspection:** You or `copilot` inspect the workspace diff via `inspect_agent_workspace(agent = "coder")`.
4. **Merge & Verification:** `merge_agent_workspace(agent = "coder")` checks whether declarative gates in `project.toml` pass. If green, it merges changes back into `@`. If quality checks fail, it cleanly aborts and restores without data loss.
