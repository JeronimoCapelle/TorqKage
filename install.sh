#!/usr/bin/env bash
# ==============================================================================
# agy-suite Bundler & Installer
#
# Installs agy-suite into ~/.gemini/config/plugins/agy-suite and symlinks CLI tools
# to ~/.local/bin/agy-ctrl, agy-mcp, and agy-hooks.
#
# By default, downloads precompiled, high-performance binaries (built with the
# 'perf' profile: fat LTO, stripped, <1ms startup) from GitHub Releases, avoiding
# any Rust/Cargo compilation requirement for end users.
# ==============================================================================

set -euo pipefail

GITHUB_REPO="JeronimoCapelle/agy-suite"
SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]:-$0}")" >/dev/null 2>&1 && pwd || pwd)"
TARGET_PLUGIN_DIR="${HOME}/.gemini/config/plugins/agy-suite"
LOCAL_BIN_DIR="${HOME}/.local/bin"

# Options
BUILD_SOURCE=false
FORCE_PRECOMPILED=false
PROFILE="perf"
LOCAL_ONLY=false
SKIP_SYMLINKS=false
RELEASE_TAG="latest"

print_usage() {
    cat <<EOF
Usage: ./install.sh [OPTIONS]

Options:
  --precompiled                  Install precompiled binary release from GitHub (default)
  --build-from-source            Force compiling from source using Cargo
  --profile <perf|release|fast>  Compilation profile when building from source (default: perf)
  --perf                         Shortcut for '--profile perf'
  --fast                         Shortcut for '--profile fast' (uses cargo-fast if available)
  --version <tag>                Specific release version to download (default: latest)
  --local-only                   Only stage into ./plugins/agy-suite/bin/ without global install
  --no-symlinks                  Install to ~/.gemini/config/plugins/agy-suite but skip ~/.local/bin symlinks
  -h, --help                     Display this help message
EOF
}

while [[ $# -gt 0 ]]; do
    case "$1" in
        --precompiled)
            FORCE_PRECOMPILED=true
            shift
            ;;
        --build-from-source|--source)
            BUILD_SOURCE=true
            shift
            ;;
        --profile)
            PROFILE="$2"
            shift 2
            ;;
        --perf)
            PROFILE="perf"
            shift
            ;;
        --fast)
            PROFILE="fast"
            shift
            ;;
        --version)
            RELEASE_TAG="$2"
            shift 2
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

detect_target() {
    local arch
    arch="$(uname -m)"
    case "${arch}" in
        x86_64|amd64) TARGET_ARCH="x86_64" ;;
        aarch64|arm64) TARGET_ARCH="aarch64" ;;
        *) TARGET_ARCH="${arch}" ;;
    esac

    local os
    os="$(uname -s)"
    case "${os}" in
        Linux) TARGET_OS="unknown-linux" ;;
        *) TARGET_OS="unknown-linux" ;;
    esac

    local libc="gnu"
    if command -v ldd >/dev/null 2>&1; then
        if ldd --version 2>&1 | grep -qi "musl"; then
            libc="musl"
        fi
    else
        libc="musl"
    fi

    echo "${TARGET_ARCH}-${TARGET_OS}-${libc}"
}

install_bundle_to_system() {
    local plugin_manifest_dir="$1"
    local bin_source_dir="$2"

    echo "--> Synchronizing bundle to Gemini plugin root: ${TARGET_PLUGIN_DIR}..."
    if [[ -L "${TARGET_PLUGIN_DIR}" ]]; then
        echo "--> Removing existing legacy symlink at ${TARGET_PLUGIN_DIR}..."
        rm -f "${TARGET_PLUGIN_DIR}"
    fi
    mkdir -p "${TARGET_PLUGIN_DIR}/bin"
    rm -rf "${TARGET_PLUGIN_DIR}/agents" && mkdir -p "${TARGET_PLUGIN_DIR}/agents"
    mkdir -p "${TARGET_PLUGIN_DIR}/assets"

    # Copy declarative files and manifests
    cp "${plugin_manifest_dir}/plugin.json" "${TARGET_PLUGIN_DIR}/"
    cp "${plugin_manifest_dir}/tools.toml" "${TARGET_PLUGIN_DIR}/"
    cp "${plugin_manifest_dir}/hooks.toml" "${TARGET_PLUGIN_DIR}/"
    cp "${plugin_manifest_dir}/hooks.json" "${TARGET_PLUGIN_DIR}/"
    rm -f "${TARGET_PLUGIN_DIR}/mcp_config.json"
    if [[ -f "${plugin_manifest_dir}/project.toml" ]]; then
        cp "${plugin_manifest_dir}/project.toml" "${TARGET_PLUGIN_DIR}/"
    fi
    if [[ -f "${plugin_manifest_dir}/README.md" ]]; then
        cp "${plugin_manifest_dir}/README.md" "${TARGET_PLUGIN_DIR}/"
    fi
    if [[ -d "${plugin_manifest_dir}/agents" ]]; then
        cp -r "${plugin_manifest_dir}/agents/"* "${TARGET_PLUGIN_DIR}/agents/"
    fi
    if [[ -d "${plugin_manifest_dir}/assets" ]]; then
        cp -r "${plugin_manifest_dir}/assets/"* "${TARGET_PLUGIN_DIR}/assets/"
    fi

    # Copy executables (unlink first to prevent ETXTBSY if active daemons/TUI are running)
    rm -f "${TARGET_PLUGIN_DIR}/bin/agy-mcp" "${TARGET_PLUGIN_DIR}/bin/agy-hooks" "${TARGET_PLUGIN_DIR}/bin/agy-ctrl" "${TARGET_PLUGIN_DIR}/bin/graphtrail" "${TARGET_PLUGIN_DIR}/bin/perfetto-mcp-rs" "${TARGET_PLUGIN_DIR}/bin/cargo-fast"
    cp "${bin_source_dir}/agy-mcp" "${TARGET_PLUGIN_DIR}/bin/"
    cp "${bin_source_dir}/agy-hooks" "${TARGET_PLUGIN_DIR}/bin/"
    cp "${bin_source_dir}/agy-ctrl" "${TARGET_PLUGIN_DIR}/bin/"
    if [[ -f "${bin_source_dir}/cargo-fast" ]]; then
        cp "${bin_source_dir}/cargo-fast" "${TARGET_PLUGIN_DIR}/bin/"
    elif [[ -f "${plugin_manifest_dir}/bin/cargo-fast" ]]; then
        cp "${plugin_manifest_dir}/bin/cargo-fast" "${TARGET_PLUGIN_DIR}/bin/"
    fi
    if [[ -f "${bin_source_dir}/graphtrail" ]]; then
        cp "${bin_source_dir}/graphtrail" "${TARGET_PLUGIN_DIR}/bin/"
    elif [[ -f "${plugin_manifest_dir}/bin/graphtrail" ]]; then
        cp "${plugin_manifest_dir}/bin/graphtrail" "${TARGET_PLUGIN_DIR}/bin/"
    fi
    if [[ -f "${bin_source_dir}/perfetto-mcp-rs" ]]; then
        cp "${bin_source_dir}/perfetto-mcp-rs" "${TARGET_PLUGIN_DIR}/bin/"
    elif [[ -f "${plugin_manifest_dir}/bin/perfetto-mcp-rs" ]]; then
        cp "${plugin_manifest_dir}/bin/perfetto-mcp-rs" "${TARGET_PLUGIN_DIR}/bin/"
    fi
    chmod +x "${TARGET_PLUGIN_DIR}/bin"/*

    # Create convenient user symlinks in ~/.local/bin
    if [[ "${SKIP_SYMLINKS}" = false ]]; then
        echo "--> Configuring user CLI symlinks in ${LOCAL_BIN_DIR}..."
        mkdir -p "${LOCAL_BIN_DIR}"
        ln -sf "${TARGET_PLUGIN_DIR}/bin/agy-ctrl" "${LOCAL_BIN_DIR}/agy-ctrl"
        ln -sf "${TARGET_PLUGIN_DIR}/bin/agy-mcp" "${LOCAL_BIN_DIR}/agy-mcp"
        ln -sf "${TARGET_PLUGIN_DIR}/bin/agy-hooks" "${LOCAL_BIN_DIR}/agy-hooks"
        if [[ -f "${TARGET_PLUGIN_DIR}/bin/graphtrail" ]]; then
            ln -sf "${TARGET_PLUGIN_DIR}/bin/graphtrail" "${LOCAL_BIN_DIR}/graphtrail"
        fi
        if [[ -f "${TARGET_PLUGIN_DIR}/bin/perfetto-mcp-rs" ]]; then
            ln -sf "${TARGET_PLUGIN_DIR}/bin/perfetto-mcp-rs" "${LOCAL_BIN_DIR}/perfetto-mcp-rs"
        fi
        if [[ -f "${TARGET_PLUGIN_DIR}/bin/cargo-fast" ]]; then
            ln -sf "${TARGET_PLUGIN_DIR}/bin/cargo-fast" "${LOCAL_BIN_DIR}/cargo-fast"
        fi
    fi

    # Validate installed plugin with agy CLI if present
    if command -v agy >/dev/null 2>&1; then
        echo "--> Validating installed plugin with 'agy plugin validate'..."
        agy plugin validate "${TARGET_PLUGIN_DIR}"
    else
        echo "--> Advisory: 'agy' command not found in PATH; skipping plugin validation."
    fi

    echo "================================================================================"
    echo "                   AGY-SUITE INSTALLATION COMPLETE                              "
    echo "================================================================================"
    echo "Plugin Bundle Location: ${TARGET_PLUGIN_DIR}"
    echo "Binaries Installed:     agy-mcp, agy-hooks, agy-ctrl (Precompiled / High-Perf)"
    if [[ "${SKIP_SYMLINKS}" = false ]]; then
    echo "User CLI Symlinks:     ${LOCAL_BIN_DIR}/agy-ctrl -> ${TARGET_PLUGIN_DIR}/bin/agy-ctrl"
    echo "                       ${LOCAL_BIN_DIR}/agy-mcp -> ${TARGET_PLUGIN_DIR}/bin/agy-mcp"
    echo "                       ${LOCAL_BIN_DIR}/agy-hooks -> ${TARGET_PLUGIN_DIR}/bin/agy-hooks"
    fi
    echo "Workspace State Path:  <project_root>/.agy/ (Cleanly isolated)"
    echo "================================================================================"
}

try_precompiled_install() {
    local target
    target="$(detect_target)"
    echo "==> [agy-suite] Checking for precompiled binary release for '${target}' (tag: ${RELEASE_TAG})..."

    local download_url
    if [[ "${RELEASE_TAG}" == "latest" ]]; then
        download_url="https://github.com/${GITHUB_REPO}/releases/latest/download/agy-suite-${target}.tar.gz"
    else
        download_url="https://github.com/${GITHUB_REPO}/releases/download/${RELEASE_TAG}/agy-suite-${target}.tar.gz"
    fi

    local tmp_dir
    tmp_dir="$(mktemp -d)"
    trap 'rm -rf "${tmp_dir}"' RETURN

    local tar_path="${tmp_dir}/agy-suite.tar.gz"
    echo "--> Downloading release archive from ${download_url}..."
    if ! curl -sSLf "${download_url}" -o "${tar_path}" 2>/dev/null; then
        echo "--> Notice: Precompiled release asset '${download_url}' not found."
        return 1
    fi

    echo "--> Unpacking precompiled release archive..."
    tar -xzf "${tar_path}" -C "${tmp_dir}"

    local bin_dir="${tmp_dir}"
    if [[ -d "${tmp_dir}/agy-suite-${target}" ]]; then
        bin_dir="${tmp_dir}/agy-suite-${target}"
    fi
    if [[ -d "${bin_dir}/bin" ]]; then
        bin_dir="${bin_dir}/bin"
    fi

    # Verify binaries exist in extracted archive
    for b in agy-ctrl agy-mcp agy-hooks; do
        if [[ ! -f "${bin_dir}/${b}" ]]; then
            echo "--> Notice: Archive did not contain expected binary '${b}'."
            return 1
        fi
    done

    # Determine plugin manifest location
    local plugin_manifest_dir="${SCRIPT_DIR}/plugins/agy-suite"
    if [[ ! -f "${plugin_manifest_dir}/plugin.json" ]]; then
        echo "--> Downloading plugin manifests and agent profiles from repository..."
        mkdir -p "${tmp_dir}/repo"
        curl -sSLf "https://github.com/${GITHUB_REPO}/archive/refs/heads/main.tar.gz" | tar -xz -C "${tmp_dir}/repo" --strip-components=1
        plugin_manifest_dir="${tmp_dir}/repo/plugins/agy-suite"
    fi

    install_bundle_to_system "${plugin_manifest_dir}" "${bin_dir}"
    return 0
}

build_from_source_install() {
    local workspace_root="${SCRIPT_DIR}"

    # If executed standalone via 'curl ... | bash' without repository clone
    if [[ ! -f "${workspace_root}/Cargo.toml" || ! -d "${workspace_root}/plugins/agy-suite" ]]; then
        echo "==> [agy-suite] Remote source bootstrap requested (no local repository clone detected)..."
        if ! command -v git >/dev/null 2>&1; then
            echo "Error: 'git' is required to shallow-clone agy-suite from source." >&2
            exit 1
        fi
        local tmp_clone
        tmp_clone="$(mktemp -d)"
        trap 'rm -rf "${tmp_clone}"' RETURN
        echo "--> Shallow cloning agy-suite into ${tmp_clone}/agy-suite..."
        git clone --depth 1 "https://github.com/${GITHUB_REPO}.git" "${tmp_clone}/agy-suite"
        workspace_root="${tmp_clone}/agy-suite"
    fi

    if ! command -v cargo >/dev/null 2>&1; then
        echo "Error: 'cargo' is not installed, and no precompiled release could be retrieved." >&2
        echo "Please install Rust toolchain via https://rustup.rs or specify a valid release tag." >&2
        exit 1
    fi

    local plugin_dir="${workspace_root}/plugins/agy-suite"
    echo "==> [agy-suite] Compiling binaries from source (Profile: ${PROFILE})..."

    cd "${workspace_root}"
    local bin_source_dir
    case "${PROFILE}" in
        perf)
            echo "--> Building high-performance binaries with 'cargo build --profile perf' (fat LTO, stripped)..."
            cargo build --profile perf -p agy-mcp -p agy-hooks -p agy-ctrl
            bin_source_dir="${workspace_root}/target/perf"
            ;;
        release)
            echo "--> Building release binaries with standard cargo..."
            cargo build --release -p agy-mcp -p agy-hooks -p agy-ctrl
            bin_source_dir="${workspace_root}/target/release"
            ;;
        fast)
            if [[ -x "${workspace_root}/plugins/agy-suite/bin/cargo-fast" ]]; then
                echo "--> Building fast profile with local cargo-fast..."
                "${workspace_root}/plugins/agy-suite/bin/cargo-fast" build -p agy-mcp -p agy-hooks -p agy-ctrl
            elif command -v cargo-fast >/dev/null 2>&1; then
                echo "--> Building fast profile with cargo-fast..."
                cargo-fast build -p agy-mcp -p agy-hooks -p agy-ctrl
            else
                echo "--> Notice: cargo-fast not found; building with 'cargo build --profile fast'..."
                cargo build --profile fast -p agy-mcp -p agy-hooks -p agy-ctrl
            fi
            bin_source_dir="${workspace_root}/target/fast"
            ;;
        dev|debug)
            echo "--> Building dev/debug binaries..."
            cargo build -p agy-mcp -p agy-hooks -p agy-ctrl
            bin_source_dir="${workspace_root}/target/debug"
            ;;
        *)
            echo "Error: Unknown profile '${PROFILE}'. Use 'perf', 'release', 'fast', or 'dev'." >&2
            exit 1
            ;;
    esac

    # Verify compiled binaries
    for bin in agy-mcp agy-hooks agy-ctrl; do
        if [[ ! -f "${bin_source_dir}/${bin}" ]]; then
            echo "Error: Expected compiled binary '${bin_source_dir}/${bin}' not found!" >&2
            exit 1
        fi
    done

    # Stage binaries into local workspace plugin bundle: ./plugins/agy-suite/bin/
    echo "--> Staging binaries into workspace plugin bundle: ${plugin_dir}/bin/..."
    mkdir -p "${plugin_dir}/bin"
    rm -f "${plugin_dir}/bin/agy-mcp" "${plugin_dir}/bin/agy-hooks" "${plugin_dir}/bin/agy-ctrl"
    cp "${bin_source_dir}/agy-mcp" "${plugin_dir}/bin/"
    cp "${bin_source_dir}/agy-hooks" "${plugin_dir}/bin/"
    cp "${bin_source_dir}/agy-ctrl" "${plugin_dir}/bin/"
    chmod +x "${plugin_dir}/bin"/*

    if [[ "${LOCAL_ONLY}" = true ]]; then
        if command -v agy >/dev/null 2>&1; then
            echo "--> Validating local workspace bundle with 'agy plugin validate'..."
            agy plugin validate "${plugin_dir}"
        fi
        echo "==> [agy-suite] Local workspace bundle complete (--local-only). Files staged in ${plugin_dir}/bin/."
        return 0
    fi

    install_bundle_to_system "${plugin_dir}" "${bin_source_dir}"
}

# Execution Entry Point
if [[ "${BUILD_SOURCE}" = true ]]; then
    build_from_source_install
else
    # Try precompiled release first
    if try_precompiled_install; then
        exit 0
    fi

    if [[ "${FORCE_PRECOMPILED}" = true ]]; then
        echo "Error: Precompiled binary installation was explicitly requested (--precompiled), but download failed." >&2
        exit 1
    fi

    echo "==> [agy-suite] Precompiled release not available; falling back to building from source..."
    build_from_source_install
fi
