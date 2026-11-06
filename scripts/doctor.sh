#!/usr/bin/env bash
# Checks an existing installation. Exit 0 = healthy, 1 = something is wrong.
# Usage: scripts/doctor.sh [--prefix <dir>] [--agent-dir <dir>] [--config-dir <dir>]
set -uo pipefail
. "$(dirname "$(readlink -f "${BASH_SOURCE[0]}")")/lib.sh"
while [ $# -gt 0 ]; do
  parse_common_flag "$@" && rc=0 || rc=$?
  [ "$rc" = 0 ] && die "unknown option: $1"; shift "$rc"
done
bad=0; fail() { err "$*"; bad=1; }

step "Installed files"
[ -x "$PREFIX/bin/gs3" ]     && ok "$PREFIX/bin/gs3"     || fail "missing $PREFIX/bin/gs3 (run ./install.sh)"
[ -x "$PREFIX/bin/gs3-hud" ] && ok "$PREFIX/bin/gs3-hud" || fail "missing $PREFIX/bin/gs3-hud"
[ -d "$CONFIG_DIR" ]         && ok "config dir $CONFIG_DIR" || warn "no config dir $CONFIG_DIR (the HUD then uses defaults and cannot save settings)"
bash -n "$PREFIX/bin/gs3" 2>/dev/null && ok "launcher syntax" || fail "launcher has a syntax error"

step "Sidebar runs"
if [ -x "$PREFIX/bin/gs3-hud" ]; then
  # The binary starts and parses its arguments.
  "$PREFIX/bin/gs3-hud" --help 2>&1 | grep -q -- "--pane" && ok "gs3-hud starts (--help)" || fail "gs3-hud --help failed"
  # A real render needs a live herdr pane: --once prints nothing for a pane that does not exist.
  if [ "${HERDR_ENV:-}" = 1 ] && [ -n "${HERDR_PANE_ID:-}" ]; then
    out="$("$PREFIX/bin/gs3-hud" --pane "$HERDR_PANE_ID" --cwd "$PWD" --once 2>&1)"
    grep -q "GENTLE-SHELL" <<<"$out" && ok "gs3-hud --once rendered against pane $HERDR_PANE_ID" || fail "gs3-hud --once printed no header for pane $HERDR_PANE_ID"
  else
    warn "render check skipped: run doctor.sh inside herdr to render against a real pane"
  fi
fi

step "Pi extensions"
for e in "${EXTENSIONS[@]}"; do
  [ -f "$EXT_DIR/$e/index.ts" ] && ok "$e" || warn "$e not found in $EXT_DIR"
done
for e in "${EXTENSION_FILES[@]}"; do [ -f "$EXT_DIR/$e" ] && ok "$e" || warn "$e not found in $EXT_DIR"; done

step "Environment"
have herdr && ok "herdr found" || fail "herdr not found in PATH"
have jq    && ok "jq found"    || fail "jq not found in PATH"
[ "${HERDR_ENV:-}" = 1 ] && ok "this shell runs inside herdr" || warn "this shell is not inside herdr (fine: gs3 creates a workspace for you)"

[ "$bad" = 0 ] && { printf '\nHealthy.\n'; exit 0; }
printf '\nProblems found.\n'; exit 1
