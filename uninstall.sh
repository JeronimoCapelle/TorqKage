#!/usr/bin/env bash
# ==============================================================================
# TorqKage Uninstaller
#
# Removes the TorqKage plugin bundle and CLI symlinks cleanly:
#   1. Removes ~/.gemini/config/plugins/torqkage
#   2. Removes ~/.local/bin/tkg, ~/.local/bin/tkg-mcp, ~/.local/bin/tkg-hooks
#   3. Optionally cleans local workspace bundle ./plugins/torqkage/bin
# ==============================================================================

set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
TARGET_PLUGIN_DIR="${HOME}/.gemini/config/plugins/torqkage"
EXTRA_PLUGIN_DIRS=(
    "${HOME}/.local/share/opencode/plugins/torqkage"
    "${HOME}/.claude/plugins/torqkage"
    "${HOME}/.config/tkg/plugins/torqkage"
)
LOCAL_BIN_DIR="${HOME}/.local/bin"

echo "==> [TorqKage] Removing plugin bundle and symlinks..."

# Remove CLI symlinks
for bin in tkg tkg-mcp tkg-hooks; do
    if [[ -L "${LOCAL_BIN_DIR}/${bin}" ]]; then
        echo "--> Removing symlink ${LOCAL_BIN_DIR}/${bin}..."
        rm -f "${LOCAL_BIN_DIR}/${bin}"
    fi
done

# Remove plugin bundle
if [[ -d "${TARGET_PLUGIN_DIR}" ]]; then
    echo "--> Removing plugin directory ${TARGET_PLUGIN_DIR}..."
    rm -rf "${TARGET_PLUGIN_DIR}"
fi

# Remove bundles installed for other harnesses
for extra in "${EXTRA_PLUGIN_DIRS[@]}"; do
    if [[ -d "${extra}" ]]; then
        echo "--> Removing plugin directory ${extra}..."
        rm -rf "${extra}"
    fi
done

# Clean local workspace bin if exists
if [[ -d "${SCRIPT_DIR}/plugins/torqkage/bin" ]]; then
    echo "--> Cleaning local workspace bundle ${SCRIPT_DIR}/plugins/torqkage/bin..."
    rm -rf "${SCRIPT_DIR}/plugins/torqkage/bin"
fi

echo "================================================================================"
echo "                   TORQKAGE UNINSTALL COMPLETE                                  "
echo "================================================================================"
echo "All plugin bundles, executables, and symlinks have been removed."
echo "Note: Workspace .tkg/ state directories were preserved."
echo "================================================================================"
