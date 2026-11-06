#!/usr/bin/env bash
# gs3-sidebar live test: real gs2 + HUD in a throwaway herdr workspace (closed on exit).
# Usage: live-test.sh ["prompt for gs2"]   Env: KEEP=1 keeps the workspace, GS3_REPO.
# Uses only gs2 (~/.local/bin/gentle-shell2 → ~/.gentle-shell). The prompt costs one model turn.
set -uo pipefail
S="$(cd "$(dirname "$(readlink -f "${BASH_SOURCE[0]}")")/.." && pwd)"
REPO="${GS3_REPO:-$(cd "$S/../../.." && pwd)}"
command -v herdr >/dev/null || { echo "herdr not found"; exit 1; }
prompt="${1:-}"
ws_json="$(cd "$REPO" && env -u HERDR_ENV -u HERDR_PANE_ID GS3_NO_ATTACH=1 "$REPO/gs3/bin/gs3" 2>&1)" || { echo "gs3 failed: $ws_json"; exit 1; }
ws="$(jq -r '.result.workspace.workspace_id // empty' <<<"$ws_json")"; [ -n "$ws" ] || { echo "no workspace: $ws_json"; exit 1; }
[ "${KEEP:-0}" = 1 ] || trap 'herdr workspace close "$ws" >/dev/null 2>&1; echo "closed $ws"' EXIT
echo "workspace $ws"
panes(){ herdr pane list --workspace "$ws" | jq -r '.result.panes[] | "\(.pane_id) \(.agent // "-") \(.agent_status)"'; }
gs=""; hud=""
for _ in $(seq 1 30); do
  gs="$(panes | awk '$2=="pi"{print $1; exit}')"; hud="$(panes | awk -v g="$gs" '$1!=g{print $1; exit}')"
  [ -n "$gs" ] && [ -n "$hud" ] && break; sleep 1
done
[ -n "$gs" ] && [ -n "$hud" ] || { echo "panes not ready:"; panes; exit 1; }
echo "gs2 pane $gs · hud pane $hud"
status(){ herdr pane get "$gs" | jq -r '.result.pane.agent_status'; }
if [ -n "$prompt" ]; then
  # gs2 drops input typed before its footer is drawn: wait for it.
  herdr pane wait-output "$gs" --match 'gentle shell' --timeout 60000 >/dev/null || { echo "gs2 did not become ready"; exit 1; }
  sleep 2
  herdr pane send-text "$gs" "$prompt" >/dev/null && herdr pane send-keys "$gs" Enter >/dev/null
  for _ in $(seq 1 30); do [ "$(status)" = working ] && break; sleep 1; done
  [ "$(status)" = working ] || echo "WARN  gs2 never reported working (prompt not accepted?)"
  echo "--- HUD while $(status) ---"; herdr pane read "$hud" --lines 60 | sed -n 2,2p
  for _ in $(seq 1 90); do [ "$(status)" != working ] && break; sleep 2; done
  sleep 1
fi
echo "--- HUD (status $(status)) ---"; herdr pane read "$hud" --lines 60
echo "--- gs2 footer ---"; herdr pane read "$gs" --lines 12 | grep -v '^\s*$' | tail -3
pid="$(pgrep -f "gs3-hud --pane $gs" | head -1)"
if [ -n "$pid" ]; then
  a=$(awk '{print $14+$15}' /proc/$pid/stat); sleep 10; b=$(awk '{print $14+$15}' /proc/$pid/stat)
  echo "--- idle: $((b-a)) CPU ticks/10 s · RSS $(awk '/VmRSS/{print $2" kB"}' /proc/$pid/status) · threads $(awk '/Threads/{print $2}' /proc/$pid/status)"
else echo "--- gs3-hud process not found"; fi
