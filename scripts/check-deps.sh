#!/usr/bin/env bash
# Verifies the tools gs3 needs. Exit 0 = ready, 1 = a required tool is missing.
# Usage: scripts/check-deps.sh [--no-build]
set -uo pipefail
. "$(dirname "$(readlink -f "${BASH_SOURCE[0]}")")/lib.sh"

need_build=1; [ "${1:-}" = --no-build ] && need_build=0
missing=0

req() { # req <tool> <why> <how to install>
  if have "$1"; then ok "$1 ($("$1" --version 2>/dev/null | head -1))"; else err "$1 missing: $2. $3"; missing=1; fi
}
opt() { if have "$1"; then ok "$1 (optional: $2)"; else warn "$1 not found (optional: $2)"; fi; }

step "Required tools"
req herdr "gs3 lives in a herdr pane" "see https://herdr.dev"
req jq    "the launcher parses herdr's JSON" "sudo apt install jq"
if [ "$need_build" = 1 ]; then
  req cargo "to compile gs3-hud" "https://rustup.rs  (or pass --no-build with a prebuilt binary)"
  req cc    "rusqlite compiles bundled SQLite" "sudo apt install build-essential"
fi

step "Pi (for the extensions)"
if have pi || have gentle-shell2; then ok "pi / gentle-shell2 found"; else warn "neither pi nor gentle-shell2 found: the HUD works, but there is nothing to monitor"; fi
have node && ok "node $(node --version) (only needed to run the extension tests)" || warn "node not found (only needed to run the extension tests)"

step "Optional integrations"
opt flatpak "flame button launches Flameshot as Flatpak"
opt vesktop "mic/headphones controls need Vesktop with Discord RPC"
[ -n "${XDG_RUNTIME_DIR:-}" ] && ok "XDG_RUNTIME_DIR=$XDG_RUNTIME_DIR" || warn "XDG_RUNTIME_DIR unset: voice controls cannot find Discord sockets"

step "Platform"
printf '  %s\n' "$(. /etc/os-release 2>/dev/null; printf '%s' "${PRETTY_NAME:-$(uname -s)}")  /  ${XDG_CURRENT_DESKTOP:-no desktop detected}"
[ "$(uname -s)" = Linux ] || warn "gs3 was only developed and tested on Linux (Pop!_OS + COSMIC)"

[ "$missing" = 0 ] && { printf '\nReady.\n'; exit 0; }
printf '\nInstall the missing required tools and run again.\n'; exit 1
