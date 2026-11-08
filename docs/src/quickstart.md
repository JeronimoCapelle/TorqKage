# Quickstart

Get started with `agy-suite` in under 60 seconds.

## Prerequisites

- **Linux** (x86_64 or aarch64) with `bubblewrap` (`bwrap`) installed.
- **Rust Toolchain:** Rust 1.85+ (Nightly or Rust 2024 Edition).
- **Google Antigravity CLI:** `agy` installed on `$PATH`.
- *(Optional for fast compilation):* `mold` linker and `rustc-codegen-cranelift`.

## Installation

### Method 1: Automated Installer (One-Liner)

Runs standalone without requiring you to clone the repository first:

```bash
curl -sSL https://raw.githubusercontent.com/JeronimoCapelle/agy-suite/main/install.sh | bash
```

*(Tip: If you have Cranelift and Mold installed, pass `-s -- --fast` for near-instant compilation).*

### Method 2: From Source

Clone the repository and compile using the fast Cranelift/Mold development profile:

```bash
git clone https://github.com/JeronimoCapelle/agy-suite.git
cd agy-suite
./install.sh --fast
```

The installer will:
1. Compile `agy-ctrl`, `agy-mcp`, and `agy-hooks`.
2. Bundle them into `~/.gemini/config/plugins/agy-suite`.
3. Create symlinks in `~/.local/bin/`.
4. Validate the plugin with `agy plugin validate`.

## Running the Flight Deck

Launch `agy-ctrl` inside any Git or Jujutsu repository:

```bash
agy-ctrl
```

### Key Controls

| Key | Action |
| :--- | :--- |
| <kbd>j</kbd> / <kbd>k</kbd> | Navigate agent list or tool catalog |
| <kbd>Space</kbd> | Toggle permission gate (`ALLOW` vs `ASK`) |
| <kbd>Tab</kbd> | Switch between Fleet Matrix and Tool Catalog |
| <kbd>Enter</kbd> / <kbd>r</kbd> | Launch or resume session with selected agent |
| <kbd>n</kbd> | Force launch fresh, unresumed session |
| <kbd>b</kbd> | Toggle Bubblewrap OS container isolation |
| <kbd>w</kbd> | Toggle Jujutsu isolated worktree (`.agy/workspaces/<agent>`) |
| <kbd>?</kbd> | Open paginated in-deck manual |
| <kbd>q</kbd> | Exit control deck |
