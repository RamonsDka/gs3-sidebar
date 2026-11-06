#!/usr/bin/env bash
# Copies the Pi extensions into the agent's auto-discovery directory ($AGENT_DIR/extensions).
# An existing, different extension is moved to <name>.bak.<timestamp> first; nothing is deleted.
# Usage: scripts/install-extensions.sh [--agent-dir <dir>] [--dry-run]
set -euo pipefail
. "$(dirname "$(readlink -f "${BASH_SOURCE[0]}")")/lib.sh"

while [ $# -gt 0 ]; do
  parse_common_flag "$@" && rc=0 || rc=$?
  [ "$rc" = 0 ] && die "unknown option: $1"; shift "$rc"
done

step "Pi extensions -> $EXT_DIR"
run install -d "$EXT_DIR"
stamp="$(date +%Y%m%d%H%M%S)"

place() { # place <name>  (a directory or a single file under extensions/)
  local name="$1" src="$ROOT/extensions/$1" dst="$EXT_DIR/$1"
  [ -e "$src" ] || die "missing in repo: extensions/$name"
  if [ -e "$dst" ] && ! diff -rq "$src" "$dst" >/dev/null 2>&1; then
    run mv "$dst" "$dst.bak.$stamp"; warn "kept your previous $name as $name.bak.$stamp"
  fi
  if [ -d "$src" ]; then run rm -rf "$dst"; run cp -r "$src" "$dst"; else run cp "$src" "$dst"; fi
  ok "$name"
}
for e in "${EXTENSIONS[@]}" "${EXTENSION_FILES[@]}"; do place "$e"; done
printf '\nRun /reload inside Pi (or restart it) to load them.\n'
