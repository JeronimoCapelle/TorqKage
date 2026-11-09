#!/usr/bin/env bash
# ==============================================================================
# Torqkage Uninstaller
#
# Removes the Torqkage plugin bundle and CLI symlinks cleanly:
#   1. Removes ~/.gemini/config/plugins/torqkage
#   2. Removes ~/.local/bin/tkg, ~/.local/bin/tkg-mcp, ~/.local/bin/tkg-hooks
#   3. Optionally cleans local workspace bundle ./plugins/torqkage/bin
# ==============================================================================

set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
TARGET_PLUGIN_DIR="${HOME}/.gemini/config/plugins/torqkage"
LOCAL_BIN_DIR="${HOME}/.local/bin"

echo "==> [Torqkage] Removing plugin bundle and symlinks..."

# Remove new symlinks and legacy symlinks
for bin in tkg tkg-mcp tkg-hooks agy-ctrl agy-mcp agy-hooks; do
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

# Clean legacy plugin directory if exists
if [[ -d "${HOME}/.gemini/config/plugins/agy-suite" ]]; then
    echo "--> Removing legacy plugin directory ${HOME}/.gemini/config/plugins/agy-suite..."
    rm -rf "${HOME}/.gemini/config/plugins/agy-suite"
fi

# Clean local workspace bin if exists
if [[ -d "${SCRIPT_DIR}/plugins/torqkage/bin" ]]; then
    echo "--> Cleaning local workspace bundle ${SCRIPT_DIR}/plugins/torqkage/bin..."
    rm -rf "${SCRIPT_DIR}/plugins/torqkage/bin"
fi
if [[ -d "${SCRIPT_DIR}/plugins/agy-suite/bin" ]]; then
    rm -rf "${SCRIPT_DIR}/plugins/agy-suite/bin"
fi

echo "================================================================================"
echo "                   TORQKAGE UNINSTALL COMPLETE                                  "
echo "================================================================================"
echo "All plugin bundles, executables, and symlinks have been removed."
echo "Note: Workspace .tkg/ state directories were preserved."
echo "================================================================================"
