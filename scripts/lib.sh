#!/usr/bin/env bash
# Shared helpers for the gs3-sidebar installer scripts. Source it; do not run it.

ROOT="$(cd "$(dirname "$(readlink -f "${BASH_SOURCE[0]}")")/.." && pwd)"

# Defaults, all overridable by flags or the environment.
PREFIX="${GS3_PREFIX:-$HOME/.local}"                        # binaries go to $PREFIX/bin
CONFIG_DIR="${GS3_CONFIG_DIR:-${XDG_CONFIG_HOME:-$HOME/.config}/gs3}"
AGENT_DIR="${GS3_AGENT_DIR:-${PI_CODING_AGENT_DIR:-$HOME/.pi/agent}}"
EXT_DIR="$AGENT_DIR/extensions"
EXTENSIONS=(model-picker copy-commands message-frames)      # directories
EXTENSION_FILES=(herdr-workspace-title.ts)                  # single files
DRY_RUN="${GS3_DRY_RUN:-0}"

if [ -t 1 ]; then G=$'\e[32m'; R=$'\e[31m'; Y=$'\e[33m'; B=$'\e[1m'; N=$'\e[0m'; else G=; R=; Y=; B=; N=; fi
ok()   { printf '%s✔%s %s\n' "$G" "$N" "$*"; }
warn() { printf '%s!%s %s\n' "$Y" "$N" "$*"; }
err()  { printf '%s✘%s %s\n' "$R" "$N" "$*" >&2; }
step() { printf '\n%s== %s%s\n' "$B" "$*" "$N"; }
die()  { err "$*"; exit 1; }

# run <cmd...>: execute, or only print it under --dry-run.
run() {
  if [ "$DRY_RUN" = 1 ]; then printf '  [dry-run] %s\n' "$*"; else "$@"; fi
}

have() { command -v "$1" >/dev/null 2>&1; }

# Parse flags shared by install.sh and uninstall.sh.
parse_common_flag() {
  case "$1" in
    --prefix)     PREFIX="$2"; return 2 ;;
    --agent-dir)  AGENT_DIR="$2"; EXT_DIR="$AGENT_DIR/extensions"; return 2 ;;
    --config-dir) CONFIG_DIR="$2"; return 2 ;;
    --dry-run)    DRY_RUN=1; return 1 ;;
  esac
  return 0
}
