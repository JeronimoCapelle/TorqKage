#!/usr/bin/env bash
# ==============================================================================
# agy-suite Bundler & Installer
#
# Compiles agy-suite binaries (agy-mcp, agy-hooks, agy-ctrl) and packages them
# into the self-contained plugin bundle directory:
#   1. Local workspace bundle:  ./plugins/agy-suite/bin/
#   2. Gemini config plugin:    ~/.gemini/config/plugins/agy-suite/
#   3. CLI symlinks:            ~/.local/bin/agy-ctrl (and agy-mcp, agy-hooks)
# ==============================================================================

set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
WORKSPACE_ROOT="${SCRIPT_DIR}"
PLUGIN_DIR="${WORKSPACE_ROOT}/plugins/agy-suite"
TARGET_PLUGIN_DIR="${HOME}/.gemini/config/plugins/agy-suite"
LOCAL_BIN_DIR="${HOME}/.local/bin"

PROFILE="release"
LOCAL_ONLY=false
SKIP_SYMLINKS=false

print_usage() {
    cat <<EOF
Usage: ./install.sh [OPTIONS]

Options:
  --profile <release|fast|dev>   Compilation profile to build (default: release)
  --fast                         Shortcut for '--profile fast' (uses cargo-fast if available)
  --local-only                   Only bundle into ./plugins/agy-suite/bin/ without global install
  --no-symlinks                  Install to ~/.gemini/config/plugins/agy-suite but skip ~/.local/bin symlinks
  -h, --help                     Display this help message
EOF
}

while [[ $# -gt 0 ]]; do
    case "$1" in
        --profile)
            PROFILE="$2"
            shift 2
            ;;
        --fast)
            PROFILE="fast"
            shift
            ;;
        --local-only)
            LOCAL_ONLY=true
            shift
            ;;
        --no-symlinks)
            SKIP_SYMLINKS=true
            shift
            ;;
        -h|--help)
            print_usage
            exit 0
            ;;
        *)
            echo "Unknown option: $1" >&2
            print_usage
            exit 1
            ;;
    esac
done

echo "==> [agy-suite] Starting bundle & install pipeline (Profile: ${PROFILE})..."

cd "${WORKSPACE_ROOT}"

# 1. Compile binaries according to requested profile
case "${PROFILE}" in
    release)
        echo "--> Building release binaries with standard cargo..."
        cargo build --release -p agy-mcp -p agy-hooks -p agy-ctrl
        BIN_SOURCE_DIR="${WORKSPACE_ROOT}/target/release"
        ;;
    fast)
        if command -v cargo-fast >/dev/null 2>&1; then
            echo "--> Building fast profile with cargo-fast (Cranelift + Mold)..."
            cargo fast build -p agy-mcp -p agy-hooks -p agy-ctrl
        else
            echo "--> Notice: cargo-fast not found in PATH; falling back to cargo build --profile fast..."
            cargo build --profile fast -p agy-mcp -p agy-hooks -p agy-ctrl
        fi
        BIN_SOURCE_DIR="${WORKSPACE_ROOT}/target/fast"
        ;;
    dev|debug)
        echo "--> Building dev/debug binaries..."
        cargo build -p agy-mcp -p agy-hooks -p agy-ctrl
        BIN_SOURCE_DIR="${WORKSPACE_ROOT}/target/debug"
        ;;
    *)
        echo "Error: Unknown profile '${PROFILE}'. Use 'release', 'fast', or 'dev'." >&2
        exit 1
        ;;
esac

# Verify binary existence
for bin in agy-mcp agy-hooks agy-ctrl; do
    if [[ ! -f "${BIN_SOURCE_DIR}/${bin}" ]]; then
        echo "Error: Expected compiled binary '${BIN_SOURCE_DIR}/${bin}' not found!" >&2
        exit 1
    fi
done

# 2. Stage binaries into the workspace plugin bundle: ./plugins/agy-suite/bin/
echo "--> Staging binaries into local workspace plugin bundle: ${PLUGIN_DIR}/bin/..."
mkdir -p "${PLUGIN_DIR}/bin"
cp "${BIN_SOURCE_DIR}/agy-mcp" "${PLUGIN_DIR}/bin/"
cp "${BIN_SOURCE_DIR}/agy-hooks" "${PLUGIN_DIR}/bin/"
cp "${BIN_SOURCE_DIR}/agy-ctrl" "${PLUGIN_DIR}/bin/"
chmod +x "${PLUGIN_DIR}/bin"/*

if [[ "${LOCAL_ONLY}" = true ]]; then
    if command -v agy >/dev/null 2>&1; then
        echo "--> Validating local workspace bundle with 'agy plugin validate'..."
        agy plugin validate "${PLUGIN_DIR}"
    fi
    echo "==> [agy-suite] Local workspace bundle complete (--local-only). Files staged in ${PLUGIN_DIR}/bin/."
    exit 0
fi

# 3. Synchronize plugin bundle to ~/.gemini/config/plugins/agy-suite/
echo "--> Synchronizing bundle to Gemini plugin root: ${TARGET_PLUGIN_DIR}..."
if [[ -L "${TARGET_PLUGIN_DIR}" ]]; then
    echo "--> Removing existing legacy symlink at ${TARGET_PLUGIN_DIR}..."
    rm -f "${TARGET_PLUGIN_DIR}"
fi
mkdir -p "${TARGET_PLUGIN_DIR}/bin"
mkdir -p "${TARGET_PLUGIN_DIR}/agents"
mkdir -p "${TARGET_PLUGIN_DIR}/assets"

# Copy declarative files and manifests
cp "${PLUGIN_DIR}/plugin.json" "${TARGET_PLUGIN_DIR}/"
cp "${PLUGIN_DIR}/tools.toml" "${TARGET_PLUGIN_DIR}/"
cp "${PLUGIN_DIR}/hooks.toml" "${TARGET_PLUGIN_DIR}/"
cp "${PLUGIN_DIR}/hooks.json" "${TARGET_PLUGIN_DIR}/"
cp "${PLUGIN_DIR}/mcp_config.json" "${TARGET_PLUGIN_DIR}/"
cp -r "${PLUGIN_DIR}/agents/"* "${TARGET_PLUGIN_DIR}/agents/"
cp -r "${PLUGIN_DIR}/assets/"* "${TARGET_PLUGIN_DIR}/assets/"

# Copy executables
cp "${PLUGIN_DIR}/bin/agy-mcp" "${TARGET_PLUGIN_DIR}/bin/"
cp "${PLUGIN_DIR}/bin/agy-hooks" "${TARGET_PLUGIN_DIR}/bin/"
cp "${PLUGIN_DIR}/bin/agy-ctrl" "${TARGET_PLUGIN_DIR}/bin/"
chmod +x "${TARGET_PLUGIN_DIR}/bin"/*

# 4. Create convenient symlinks in ~/.local/bin
if [[ "${SKIP_SYMLINKS}" = false ]]; then
    echo "--> Configuring user CLI symlinks in ${LOCAL_BIN_DIR}..."
    mkdir -p "${LOCAL_BIN_DIR}"
    ln -sf "${TARGET_PLUGIN_DIR}/bin/agy-ctrl" "${LOCAL_BIN_DIR}/agy-ctrl"
    ln -sf "${TARGET_PLUGIN_DIR}/bin/agy-mcp" "${LOCAL_BIN_DIR}/agy-mcp"
    ln -sf "${TARGET_PLUGIN_DIR}/bin/agy-hooks" "${LOCAL_BIN_DIR}/agy-hooks"
fi

# 5. Validate installed plugin with agy plugin validate
if command -v agy >/dev/null 2>&1; then
    echo "--> Validating installed plugin with 'agy plugin validate'..."
    agy plugin validate "${TARGET_PLUGIN_DIR}"
else
    echo "--> Advisory: 'agy' command not found in PATH; skipping plugin validation."
fi

echo "================================================================================"
echo "                   AGY-SUITE INSTALLATION & BUNDLE COMPLETE                     "
echo "================================================================================"
echo "Plugin Bundle Location: ${TARGET_PLUGIN_DIR}"
echo "Binaries Packaged:     agy-mcp, agy-hooks, agy-ctrl (Profile: ${PROFILE})"
if [[ "${SKIP_SYMLINKS}" = false ]]; then
echo "User CLI Symlinks:     ${LOCAL_BIN_DIR}/agy-ctrl -> ${TARGET_PLUGIN_DIR}/bin/agy-ctrl"
echo "                       ${LOCAL_BIN_DIR}/agy-mcp -> ${TARGET_PLUGIN_DIR}/bin/agy-mcp"
echo "                       ${LOCAL_BIN_DIR}/agy-hooks -> ${TARGET_PLUGIN_DIR}/bin/agy-hooks"
fi
echo "Workspace State Path:  <project_root>/.agy/ (Cleanly isolated)"
echo "================================================================================"
