#!/usr/bin/env bash
# gs3-sidebar installer (Linux). Runs the scripts in scripts/ in order:
#   check-deps.sh -> build-hud.sh -> install-launcher.sh -> install-extensions.sh -> doctor.sh
#
# Usage: ./install.sh [options]
#   --prefix <dir>       where bin/gs3 and bin/gs3-hud go        (default ~/.local)
#   --agent-dir <dir>    Pi agent dir; extensions go in <dir>/extensions   (default ~/.pi/agent)
#   --config-dir <dir>   HUD settings directory                   (default ~/.config/gs3)
#   --binary <file>      install a prebuilt gs3-hud instead of compiling with cargo
#   --no-extensions      install only the sidebar, skip the Pi extensions
#   --dry-run            print what would happen, change nothing
#   -y, --yes            do not ask for confirmation
#   -h, --help
set -euo pipefail
S="$(cd "$(dirname "$(readlink -f "${BASH_SOURCE[0]}")")/scripts" && pwd)"
. "$S/lib.sh"

BINARY=""; WITH_EXT=1; YES=0; pass=()
while [ $# -gt 0 ]; do
  case "$1" in
    -h|--help) sed -n '2,13p' "${BASH_SOURCE[0]}" | sed 's/^# \{0,1\}//'; exit 0 ;;
    --binary) BINARY="$2"; shift 2 ;;
    --no-extensions) WITH_EXT=0; shift ;;
    -y|--yes) YES=1; shift ;;
    *) parse_common_flag "$@" && rc=0 || rc=$?
       [ "$rc" = 0 ] && die "unknown option: $1 (try --help)"
       pass+=("${@:1:$rc}"); shift "$rc" ;;
  esac
done

[ "$(uname -s)" = Linux ] || die "this installer targets Linux. For Windows see docs/windows.md"

printf '%sgs3-sidebar%s will install:\n' "$B" "$N"
printf '  %s/bin/gs3, %s/bin/gs3-hud\n  %s\n' "$PREFIX" "$PREFIX" "$CONFIG_DIR"
[ "$WITH_EXT" = 1 ] && printf '  Pi extensions in %s\n' "$EXT_DIR"
if [ "$YES" != 1 ] && [ "$DRY_RUN" != 1 ] && [ -t 0 ]; then
  read -r -p "Continue? [y/N] " a; [[ "$a" =~ ^[yYsS]$ ]] || die "cancelled"
fi

build_args=(); [ -n "$BINARY" ] && build_args=(--binary "$BINARY")
"$S/check-deps.sh" $([ -n "$BINARY" ] && echo --no-build) || die "fix the missing tools above and run again"
"$S/build-hud.sh" "${build_args[@]}" "${pass[@]}"
"$S/install-launcher.sh" "${pass[@]}"
[ "$WITH_EXT" = 1 ] && "$S/install-extensions.sh" "${pass[@]}"

[ "$DRY_RUN" = 1 ] && { printf '\nDry run finished. Nothing was changed.\n'; exit 0; }
step "Verification"
"$S/doctor.sh" "${pass[@]}" || warn "doctor reported problems (see above); the install itself finished"
printf '\n%sDone.%s Open a terminal inside herdr and run: gs3\n' "$G" "$N"
