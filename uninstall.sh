#!/usr/bin/env bash
# Removes what install.sh put in place. Never touches your Pi sessions or settings.
# Usage: ./uninstall.sh [--prefix <dir>] [--agent-dir <dir>] [--config-dir <dir>] [--purge-config] [--dry-run]
#   --purge-config   also delete the HUD settings directory (kept by default)
set -euo pipefail
. "$(dirname "$(readlink -f "${BASH_SOURCE[0]}")")/scripts/lib.sh"

PURGE=0
while [ $# -gt 0 ]; do
  case "$1" in
    -h|--help) sed -n '2,5p' "${BASH_SOURCE[0]}" | sed 's/^# \{0,1\}//'; exit 0 ;;
    --purge-config) PURGE=1; shift ;;
    *) parse_common_flag "$@" && rc=0 || rc=$?
       [ "$rc" = 0 ] && die "unknown option: $1 (try --help)"; shift "$rc" ;;
  esac
done

step "Removing gs3-sidebar"
for f in "$PREFIX/bin/gs3" "$PREFIX/bin/gs3-hud"; do
  if [ -e "$f" ]; then run rm -f "$f"; ok "removed $f"; else warn "not present: $f"; fi
done
for e in "${EXTENSIONS[@]}" "${EXTENSION_FILES[@]}"; do
  if [ -e "$EXT_DIR/$e" ]; then run rm -rf "$EXT_DIR/$e"; ok "removed extension $e"; else warn "not present: extension $e"; fi
done
if [ "$PURGE" = 1 ]; then
  [ -d "$CONFIG_DIR" ] && { run rm -rf "$CONFIG_DIR"; ok "removed $CONFIG_DIR"; }
else
  [ -d "$CONFIG_DIR" ] && warn "kept $CONFIG_DIR (HUD settings and, if you used voice, discord-token.json). Use --purge-config to delete it"
fi
ls "$EXT_DIR"/*.bak.* >/dev/null 2>&1 && warn "backups of your previous extensions remain in $EXT_DIR (*.bak.*): restore them by hand if you want"
printf '\nRun /reload inside Pi to unload the extensions.\n'
