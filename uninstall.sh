#!/usr/bin/env bash
# ==============================================================================
# agy-suite Uninstaller
#
# Removes the agy-suite plugin bundle and CLI symlinks cleanly:
#   1. Removes ~/.gemini/config/plugins/agy-suite
#   2. Removes ~/.local/bin/agy-ctrl, ~/.local/bin/agy-mcp, ~/.local/bin/agy-hooks
#   3. Optionally cleans local workspace bundle ./plugins/agy-suite/bin
# ==============================================================================

set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
TARGET_PLUGIN_DIR="${HOME}/.gemini/config/plugins/agy-suite"
LOCAL_BIN_DIR="${HOME}/.local/bin"

echo "==> [agy-suite] Removing plugin bundle and symlinks..."

# Remove symlinks
for bin in agy-ctrl agy-mcp agy-hooks; do
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

# Clean local workspace bin if exists
if [[ -d "${SCRIPT_DIR}/plugins/agy-suite/bin" ]]; then
    echo "--> Cleaning local workspace bundle ${SCRIPT_DIR}/plugins/agy-suite/bin..."
    rm -rf "${SCRIPT_DIR}/plugins/agy-suite/bin"
fi

echo "================================================================================"
echo "                   AGY-SUITE UNINSTALL COMPLETE                                 "
echo "================================================================================"
echo "All plugin bundles, executables, and symlinks have been removed."
echo "Note: Workspace .agy/ state directories were preserved."
echo "================================================================================"
