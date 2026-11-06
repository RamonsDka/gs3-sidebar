#!/usr/bin/env bash
# Installs the `gs3` launcher to $PREFIX/bin and creates the config directory.
# Usage: scripts/install-launcher.sh [--prefix <dir>] [--config-dir <dir>] [--dry-run]
set -euo pipefail
. "$(dirname "$(readlink -f "${BASH_SOURCE[0]}")")/lib.sh"

while [ $# -gt 0 ]; do
  parse_common_flag "$@" && rc=0 || rc=$?
  [ "$rc" = 0 ] && die "unknown option: $1"; shift "$rc"
done

step "gs3 launcher"
run install -d "$PREFIX/bin" "$CONFIG_DIR"
run install -m 0755 "$ROOT/gs3/bin/gs3" "$PREFIX/bin/gs3"
ok "installed $PREFIX/bin/gs3"
ok "config directory $CONFIG_DIR (gs3-hud.json is written by the HUD's ⛭ tab)"
case ":$PATH:" in
  *":$PREFIX/bin:"*) ;;
  *) warn "$PREFIX/bin is not in your PATH. Add: export PATH=\"$PREFIX/bin:\$PATH\"" ;;
esac
