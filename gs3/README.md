# gs3 — Gentle Shell + telemetry sidecar

One command opens Gentle Shell (the isolated `gs2` install) in a large herdr pane
and a small read-only telemetry pane (`gs3-hud`) bound to that exact Pi session.

```
┌──────────────────────────── herdr workspace ───────────────────────────┐
│ gs2 (gentle-shell2)                          ~75%  │ gs3-hud      ~25% │
│                                                    │ ⌁ - - ⌁ banner    │
│                                                    │ MAIN GIT PROF MEM │
│                                                    │ TREE CONFIG tabs  │
└────────────────────────────────────────────────────┴───────────────────┘
```

## Usage

```sh
gs3            # inside herdr: split current pane; outside: new herdr workspace + attach
gs3 --help
```

Arguments after `gs3` are passed to `gentle-shell2`.

| Situation | Behavior |
| --- | --- |
| Run inside herdr | Splits the current pane right (ratio 0.75, no focus change), starts the sidecar bound to this pane, then runs gs2 here. |
| Run outside herdr | Starts the herdr server if needed, creates workspace `gs3 · <repo>` at `$PWD`, runs gs3 inside it, attaches. |
| gs2 exits | The sidecar pane is closed; you stay in the pane's shell. |
| Several sessions | Each `gs3` owns its own pane pair. Switching herdr workspace/tab switches both terminals together. |
| `/resume` or `/new` inside Pi | herdr reports the new session file; the sidecar follows it without restarting. |
| Sidecar not built | gs2 runs alone with a warning. |

Environment overrides: `GS3_SHELL` (big-pane command), `GS3_HUD` (sidecar binary),
`GS3_RATIO` (default `0.75`), `GS3_NO_ATTACH=1` (outside herdr: create, do not attach).

## Build

```sh
cd gs3/hud && CARGO_TARGET_DIR=../../target cargo build --release
# binary: target/release/gs3-hud  (no system OpenSSL needed)
```

## How the sidecar refreshes (event-driven, idle = no work)

| Data | Trigger |
| --- | --- |
| Agent status, session file | herdr socket `events.subscribe` (`pane.updated`, `pane.closed`, `pane.agent_status_changed`) |
| Tokens, cache, context, cost | inotify on the Pi session JSONL; only appended bytes are read |
| Git branch/dirty/ahead/commits | inotify on `.git/HEAD`, `.git/index`, `.git/refs` (debounced) and agent turn end |
| Engram observations | start and agent turn end (read-only SQLite) |
| Tool-call counts | same JSONL tail as tokens |
| MCP servers, PROF roster | session change, turn end and `r` (`mcp.json` names only; `settings.json` default model/thinking, `agents/*.md` frontmatter) |
| Stack versions | once at start |
| Model picker (MAIN model/effort rows, `sin modelo`, or Status `▸`) | explicit click → focus bound gs2 pane + fixed F9; off-render worker, triggers within 1 s ignored, failures silent; requires model-picker extension in gs2 |
| Vesktop voice buttons (MAIN) | local RPC subscription `VOICE_SETTINGS_UPDATE` (pushed); reconnect every 2 s while no client runs; 🎙/🎧 clicks send `SET_VOICE_SETTINGS`, ⚙ runs `vesktop` to open or focus its window; the flame icon runs `flameshot gui` |
| Project tree (TREE tab) | only while the tab is shown: entering it, folder toggle, Git change, turn end, `r` |
| Pull requests (`gh`) | start, HEAD change, turn end, key `r`, idle backoff 2 → 5 → 15 min |

The screen is redrawn only when state changes or the pane is resized.
The sidecar never writes to any database; its only file write is its own CONFIG file
(`~/.config/gs3/gs3-hud.json`). The sole pane-control exception is an explicit
model-section click: focus + fixed F9 to the bound gs2 pane, never another pane or keys.
Keys: `1`–`6` tabs, `r` refresh, `q` quit; other mouse clicks navigate.

Card layout: `gs3/hud/README.md`. Background and audit: `odd/tasks/gs3-sidecar.md`.
