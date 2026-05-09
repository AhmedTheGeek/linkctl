#!/usr/bin/env bash
# linkctl installer — builds in release mode, installs to ~/.cargo/bin via
# cargo install, and (optionally) adds ~/.cargo/bin to your shell PATH.
#
# Usage:
#   ./install.sh                # install + ensure ~/.cargo/bin on PATH
#   ./install.sh --no-path      # install only; don't touch shell rc
#   ./install.sh --force        # cargo install --force (reinstall)
#   ./install.sh --uninstall    # uninstall and remove the PATH entry we added

set -euo pipefail

C_RESET=$'\033[0m'
C_BOLD=$'\033[1m'
C_DIM=$'\033[2m'
C_GREEN=$'\033[32m'
C_YELLOW=$'\033[33m'
C_RED=$'\033[31m'

say()  { printf '%s==>%s %s\n'   "$C_BOLD$C_GREEN" "$C_RESET" "$*"; }
info() { printf '%s    %s%s\n'   "$C_DIM"          "$*"        "$C_RESET"; }
warn() { printf '%s!!  %s%s\n'   "$C_YELLOW"       "$*"        "$C_RESET" >&2; }
die()  { printf '%sxx  %s%s\n'   "$C_RED"          "$*"        "$C_RESET" >&2; exit 1; }

require_cmd() {
    command -v "$1" >/dev/null 2>&1 || die "missing dependency: $1"
}

# ---------- Parse args ----------
ACTION="install"
SETUP_PATH=1
EXTRA_FLAGS=()
for arg in "$@"; do
    case "$arg" in
        --uninstall|-u) ACTION="uninstall" ;;
        --force|-f)     EXTRA_FLAGS+=(--force) ;;
        --no-path)      SETUP_PATH=0 ;;
        --help|-h)
            sed -n '2,11p' "$0" | sed 's/^# \{0,1\}//'
            exit 0
            ;;
        *) die "unknown argument: $arg" ;;
    esac
done

BIN_DIR="${CARGO_HOME:-$HOME/.cargo}/bin"
MARKER="# Added by linkctl installer"

# ---------- Detect shell rc + the line we'd add ----------
detect_rc() {
    # Echoes "<rc_file>|<export_line>" for the user's shell, or "" if unknown.
    local shell_name="${SHELL##*/}"
    case "$shell_name" in
        bash) printf '%s|%s' "$HOME/.bashrc" 'export PATH="$HOME/.cargo/bin:$PATH"' ;;
        zsh)  printf '%s|%s' "${ZDOTDIR:-$HOME}/.zshrc" 'export PATH="$HOME/.cargo/bin:$PATH"' ;;
        fish) printf '%s|%s' "$HOME/.config/fish/config.fish" 'fish_add_path $HOME/.cargo/bin' ;;
        *)    printf '' ;;
    esac
}

setup_path() {
    if [[ ":$PATH:" == *":$BIN_DIR:"* ]]; then
        info "$BIN_DIR is already on PATH"
        return 0
    fi

    local detected
    detected="$(detect_rc)"
    if [[ -z "$detected" ]]; then
        warn "Unknown shell '${SHELL##*/}'. Add this line to your shell rc manually:"
        info "    export PATH=\"\$HOME/.cargo/bin:\$PATH\""
        return 0
    fi

    local rc_file="${detected%%|*}"
    local export_line="${detected#*|}"

    if [[ -f "$rc_file" ]] && grep -Fq "$MARKER" "$rc_file"; then
        info "$rc_file already has the linkctl PATH entry."
        info "Open a new shell, or run: source \"$rc_file\""
        return 0
    fi

    say "Adding $BIN_DIR to PATH in $rc_file"
    mkdir -p "$(dirname "$rc_file")"
    {
        printf '\n%s\n%s\n' "$MARKER" "$export_line"
    } >> "$rc_file"
    info "Open a new shell, or run: source \"$rc_file\""
}

remove_path_entry() {
    local detected
    detected="$(detect_rc)"
    [[ -z "$detected" ]] && return 0

    local rc_file="${detected%%|*}"
    [[ -f "$rc_file" ]] || return 0
    grep -Fq "$MARKER" "$rc_file" || return 0

    say "Removing linkctl PATH entry from $rc_file"
    cp "$rc_file" "$rc_file.bak.linkctl"
    awk -v marker="$MARKER" '
        BEGIN { skip = 0 }
        skip > 0 { skip--; next }
        index($0, marker) { skip = 1; next }
        { print }
    ' "$rc_file.bak.linkctl" > "$rc_file"
    info "Backup left at $rc_file.bak.linkctl"
}

# ---------- Uninstall ----------
if [[ "$ACTION" == "uninstall" ]]; then
    say "Uninstalling linkctl"
    require_cmd cargo
    cargo uninstall linkctl 2>/dev/null || warn "linkctl was not installed via cargo"
    remove_path_entry
    info "Config at ~/.config/linkctl/ is preserved. Remove with: rm -r ~/.config/linkctl"
    exit 0
fi

# ---------- Pre-flight ----------
say "Checking dependencies"
require_cmd cargo
require_cmd rustc
info "rustc $(rustc --version | awk '{print $2}'), cargo $(cargo --version | awk '{print $2}')"

if [[ ! -e /dev/video0 && ! -e /dev/video1 ]]; then
    warn "No /dev/video* device detected. linkctl will still install, but you'll need to plug in the camera before using it."
fi

# ---------- Migrate old config dir if present ----------
OLD_CFG="$HOME/.config/link-360-tui"
NEW_CFG="$HOME/.config/linkctl"
if [[ -d "$OLD_CFG" && ! -d "$NEW_CFG" ]]; then
    say "Migrating config: $OLD_CFG -> $NEW_CFG"
    mv "$OLD_CFG" "$NEW_CFG"
fi

# ---------- Build & install ----------
HERE="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
say "Building and installing linkctl from $HERE"
cargo install --path "$HERE" --locked "${EXTRA_FLAGS[@]}"

# ---------- Shell PATH setup ----------
if (( SETUP_PATH )); then
    setup_path
fi

# ---------- Final report ----------
if command -v linkctl >/dev/null 2>&1; then
    say "Installed: $(command -v linkctl)"
    info "Run 'linkctl' to launch, or 'linkctl --help' for options."
else
    info "Binary at: $BIN_DIR/linkctl"
    info "After opening a new shell, the 'linkctl' command should be on your PATH."
fi
