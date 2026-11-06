#!/usr/bin/env bash
# gs3-sidebar doctor: one command that proves a sidecar change is safe.
# Usage: doctor.sh [--quick]   (--quick skips cargo test/build)
# Env:   GS3_REPO (default: repo that owns this skill)
set -uo pipefail
S="$(cd "$(dirname "$(readlink -f "${BASH_SOURCE[0]}")")/.." && pwd)"
REPO="${GS3_REPO:-$(cd "$S/../.." && pwd)}"
HUD="$REPO/gs3/hud"; CORE="$HUD/src/core.rs"; UI="$HUD/src/ui.rs"; CARDS="$S/references/cards.md"
export CARGO_TARGET_DIR="$REPO/target"; BIN="$CARGO_TARGET_DIR/release/gs3-hud"
fails=0; pass(){ printf 'PASS  %s\n' "$*"; }; fail(){ printf 'FAIL  %s\n' "$*"; fails=$((fails+1)); }; warn(){ printf 'WARN  %s\n' "$*"; }
[ -f "$CORE" ] || { echo "FAIL  crate not found at $HUD (set GS3_REPO)"; exit 1; }

if [ "${1:-}" != --quick ]; then
  out="$(cd "$HUD" && cargo test --release 2>&1)"; sum="$(grep -E '^test result' <<<"$out" | tail -1)"
  if grep -q 'test result: ok' <<<"$sum"; then pass "cargo test: ${sum#test result: }"; else fail "cargo test"; grep -E 'panicked|^error|FAILED|-->' <<<"$out" | head -20; fi
  warns="$(grep -c '^warning' <<<"$out")"; [ "$warns" = 0 ] || warn "$warns compiler warnings"
  if (cd "$HUD" && cargo build --release >/dev/null 2>&1); then pass "cargo build --release"; else fail "cargo build --release"; fi
fi
bash -n "$REPO/gs3/bin/gs3" && pass "launcher syntax" || fail "launcher syntax (bash -n gs3/bin/gs3)"

# Production code only (tests may write temp files).
prod="$(for f in $(find "$HUD/src" -name '*.rs' | sort); do awk '/#\[cfg\(test\)\]/{exit}{print FILENAME":"FNR": "$0}' "$f"; done)"
# Single exception: config.rs save() writes the HUD's own --config file (temp + rename, cleanup on failure).
bad="$(grep -E 'fs::write|File::create|OpenOptions|create_dir|remove_file|remove_dir|fs::rename|SQLITE_OPEN_READ_WRITE|Connection::open\(|"send-text"|"send-keys"|"pane\.(send[a-z_]*|focus|close|run|split)"|"workspace\.(close|create)"' <<<"$prod" \
  | grep -vE '^[^:]*/src/config\.rs:[0-9]+: +(fs::write\(&tmp, body\)\.and_then\(\|_\| fs::rename\(&tmp, path\)\)|.*let _ = fs::remove_file\(&tmp\))' \
  | grep -vE '^[^:]*/src/sources\.rs:[0-9]+: +\("pane\.(focus",json!\(\{"pane_id":id\}\)\)|send_keys",json!\(\{"pane_id":id,"keys":\["f9"\]\}\)\)),?$' \
  | grep -vE '^[^:]*/src/sources/voice\.rs:[0-9]+: .*// voice-token-(save|rename)$')"
# Voice exception (approved): voice.rs save_token() writes only the private 0600 Discord token (one create_new + one rename).
for tag in voice-token-save voice-token-rename voice-gear-spawn; do
  count="$(grep -E "/src/sources/voice\.rs:[0-9]+: .*// $tag\$" <<<"$prod" | wc -l)"
  [ "$count" = 1 ] || bad+=$'\n'"voice.rs: expected exactly one $tag line, found $count"
done
# Process launches: only run() (sources.rs, bounded + timeout) and the gear launcher in voice.rs.
spawns="$(grep -E '\.spawn\(\)' <<<"$prod" | grep -vE '/src/sources/voice\.rs:[0-9]+: .*// voice-gear-spawn$' | grep -vE '/src/sources/screenshot\.rs:[0-9]+: .*// screenshot-spawn$' | grep -vE '^[^:]*/src/sources\.rs:')"
# Screenshot exception (user-approved): exactly one Flameshot launcher in screenshot.rs.
count="$(grep -E '/src/sources/screenshot\.rs:[0-9]+: .*// screenshot-spawn$' <<<"$prod" | wc -l)"
[ "$count" = 1 ] || bad+=$'\n'"screenshot.rs: expected exactly one screenshot-spawn line, found $count"
[ -z "$spawns" ] || bad+=$'\n'"unexpected process spawn: $spawns"
# The exception is exactly one fixed request of each kind, only in sources.rs.
for method in focus send_keys; do
  count="$(grep -oF "\"pane.$method\"" <<<"$prod" | wc -l)"
  [ "$count" = 1 ] || bad+=$'\n'"pane.$method: expected exactly one fixed request, found $count"
done
writers="$(grep -cE 'fs::write|fs::rename' <<<"$(grep '/src/config.rs:' <<<"$prod")")"
[ "$writers" = 1 ] || { bad+=$'\n'"config.rs: expected exactly one write+rename line, found $writers"; }
[ -z "$bad" ] && pass "read-only guard" || { fail "read-only guard: write/control API in production code"; echo "$bad" | head; }
sec="$(grep -E 'auth\.json|mcp-auth|apiKey|Authorization|mis-apis' <<<"$prod")"
[ -z "$sec" ] && pass "secret guard" || { fail "secret guard: secret-bearing names in production code"; echo "$sec" | head; }
# Allowed: herdr reconnect backoff, and run()'s wait while a subprocess is alive (try_wait + deadline).
poll="$(grep -E 'thread::sleep' <<<"$prod" | grep -vE 'backoff|try_wait')"
[ -z "$poll" ] && pass "no polling sleeps" || { fail "sleep outside reconnect backoff (polling?)"; echo "$poll"; }

# Catalog: card titles (first `seg("…")`, or `mark("Name")` = "⌁ Name", of Card::new / `let mut title`) in ui.rs
# == '## ' headings in cards.md, same order.
code_titles="$(awk '/#\[cfg\(test\)\]/{exit}{print}' "$UI" | perl -ne 'while (/(?:Card::new\("[^"]+", \[?|let mut title = )(?:vec!\[seg\("([^"]+)"|mark\("([^"]+)")/g) { print defined $1 ? "$1\n" : "⌁ $2\n" }' | uniq)"
doc_titles="$(grep -oP '^## \K.+' "$CARDS")"
if [ "$code_titles" = "$doc_titles" ]; then pass "card catalog in sync ($(wc -l <<<"$code_titles") cards)"; else
  fail "card catalog out of sync: update references/cards.md"; diff <(echo "$doc_titles") <(echo "$code_titles") | sed 's/^/      /'; fi

# Render: plain snapshot of every tab without herdr; every framed line is 1 pad + 40-column card.
if [ -x "$BIN" ]; then
  snap="$("$BIN" --pane doctor --socket /nonexistent/gs3-doctor.sock --cwd "$REPO" --once 2>/dev/null)"; rc=$?
  if [ $rc = 0 ] && [ -n "$snap" ]; then
    bad_w="$(python3 - "$snap" <<'EOF'
import sys,unicodedata
w=lambda s:sum(2 if unicodedata.east_asian_width(c) in "WF" else 0 if unicodedata.combining(c) else 1 for c in s)
for i,l in enumerate(sys.argv[1].splitlines()):
    if len(l)>1 and l[1] in "╔║╠╚" and (w(l)!=41 or l[-1] not in "╗║╣╝"): print(i+1,w(l),l)
    elif l.strip() and w(l)>42: print(i+1,w(l),l)
EOF
)"
    [ -z "$bad_w" ] && pass "--once render: every tab fits 42 columns, cards double-framed" || { fail "--once render width/frame"; echo "$bad_w" | head; }
    missing="$(while read -r t; do grep -qF "$t" <<<"$snap" || echo "$t"; done <<<"$doc_titles")"
    [ -z "$missing" ] && pass "--once shows every catalog card" || fail "--once missing cards: $(tr '\n' ' ' <<<"$missing")"
  else fail "--once exited $rc"; fi
else warn "binary not built: $BIN"; fi

# Learning reminder: source changed but no skill knowledge changed.
st="$(git -C "$REPO" status --porcelain -- gs3 skills/gs3-sidebar)"
if grep -q 'gs3/' <<<"$st" && ! grep -q 'skills/gs3-sidebar/' <<<"$st"; then
  warn "gs3 changed but the skill did not: record the learning (references/lessons.md, cards.md, architecture.md, recipes.md) + mem_save"; fi

[ $fails = 0 ] && { echo "doctor: OK"; exit 0; } || { echo "doctor: $fails failing check(s)"; exit 1; }
