# Quickstart

Get started with `Torqkage` in under 60 seconds.

## Prerequisites

- **Linux** (x86_64 or aarch64) — `bubblewrap` (`bwrap`) is optional, needed if container sandboxing is enabled.
- **Rust Toolchain:** Rust 1.85+ (Nightly or Rust 2024 Edition).
- **Google Antigravity CLI:** `agy` installed on `$PATH`.
- *(Optional for fast compilation):* `mold` linker and `rustc-codegen-cranelift`.

## Installation

### Method 1: Automated Installer (One-Liner)

Runs standalone without requiring you to clone the repository or install the Rust toolchain (fetches precompiled native binaries for your target architecture):

```bash
curl -sSL https://raw.githubusercontent.com/JeronimoCapelle/TorqKage/main/install.sh | bash
```

### Method 2: From Source

Clone the repository and compile using the high-performance `perf` profile (or `--fast` with Cranelift and Mold for development):

```bash
git clone https://github.com/JeronimoCapelle/TorqKage.git
cd Torqkage
./install.sh --perf
```

The installer will:
1. Compile `tkg`, `tkg-mcp`, and `tkg-hooks`.
2. Bundle them into `~/.gemini/config/plugins/torqkage`.
3. Create symlinks in `~/.local/bin/`.
4. Validate the plugin with `agy plugin validate`.

## Running the Flight Deck

Launch `tkg` inside any Git or Jujutsu repository:

```bash
tkg
```

### Key Controls

| Key | Action |
| :--- | :--- |
| <kbd>j</kbd> / <kbd>k</kbd> | Navigate agent list, tool catalog, or scroll content |
| <kbd>h</kbd> / <kbd>l</kbd> | Toggle focus between Agent List (left) and Tool List (right) |
| <kbd>Space</kbd> | Toggle tool authorization for selected agent |
| <kbd>Tab</kbd> / <kbd>BackTab</kbd> | Switch between Fleet Matrix, Tool Catalog, and Directives |
| <kbd>Enter</kbd> / <kbd>r</kbd> | Launch or resume session with selected agent |
| <kbd>n</kbd> | Force launch fresh, unresumed session |
| <kbd>b</kbd> / <kbd>F8</kbd> | Toggle Bubblewrap OS container isolation for `agy` session |
| <kbd>w</kbd> | Toggle Jujutsu isolated worktree (`.tkg/workspaces/<agent>`) |
| <kbd>p</kbd> | Cycle agent permission policy (Autonomous / Supervised / Plan) |
| <kbd>c</kbd> / <kbd>Shift+K</kbd> | Reset compiler failure circuit breaker strike counter |
| <kbd>g</kbd> | Toggle tool permission gate (`ALLOW` vs `ASK`) |
| <kbd>F2</kbd> / <kbd>S</kbd> | Atomic Save & Pre-flight Validation (`agy plugin validate`) |
| <kbd>F4</kbd> | Atomic Rollback unsaved changes to disk/VCS state |
| <kbd>i</kbd> | Run in-deck plugin installer and diagnostics |
| <kbd>?</kbd> / <kbd>F1</kbd> | Open in-deck manual and controls reference |
| <kbd>q</kbd> | Exit control deck |
