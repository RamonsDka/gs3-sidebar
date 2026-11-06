# gs3 sidecar architecture

## Files

| Path | Role |
| --- | --- |
| `gs3/bin/gs3` | bash launcher (alias `gs3` in `~/.bash_aliases`). Inside herdr it splits the current pane right (`--ratio 0.75 --no-focus`), runs `gs3-hud --pane <gs2 pane>` there, then execs gs2 here. Outside herdr it creates a workspace `gs3 · <repo>` and re-runs itself inside. Env: `GS3_SHELL`, `GS3_HUD`, `GS3_RATIO`, `GS3_CONFIG` (default `<repo>/.gentle-shell/gs3-hud.json`, passed as `--config` only when its folder exists), `GS3_NO_ATTACH=1`. |
| `gs3/hud/src/core.rs` | pure logic, no I/O: `Session`, `Tail` (JSONL parser), `Git`, `Engram`, `Mcp`, `State` (incl. `ui: Ui`), `Event` (incl. `Ui(Action)`, `Key(Key)`), `apply()`, `parse_wire()`, `Tone` (CUTE palette), formatters and unit tests. |
| `gs3/hud/src/ui.rs` | the CUTE rail: `Tab`, `Ui`, `Action`/`act()`, `key()`, `Seg`/`row`/`keyed`/double frame, `Card`, **`frame(&State,w,h) -> Frame{lines,hits,tab_row,scroll,max_scroll}`**, `click()`, `wheel()`, `render_lines()`, every card, tests. |
| `gs3/hud/src/config.rs` | `Config{name,hidden,collapsed}`, `load()`, atomic `save()` — the only writer. |
| `gs3/hud/src/sources.rs` | I/O: `run()` (subprocess with timeout, `GIT_OPTIONAL_LOCKS=0`), `git_snapshot`, `engram` (read-only SQLite), `mcp`, `read_tail`, herdr socket (`request`, `pane`, `herdr` subscription thread), `local` thread (inotify + git debounce), PR thread (`prs`, `pr_cycle`). |
| `gs3/hud/src/sources/voice.rs` | Vesktop-only RPC voice worker (`voice()`), token save, gear launcher; tests with a fake RPC `UnixListener`. |
| `gs3/hud/src/main.rs` | CLI (`--pane --socket --cwd --config --once`), `--once` snapshot of every visible tab, ratatui loop, keys/mouse → `Event`, keeps the last `Frame` for hit-testing, saves config when `ui.save`, Tone → RGB (+bold). |
| `gs3/hud/README.md`, `gs3/README.md` | user docs (layout table, refresh table). |
| `odd/tasks/gs3-sidecar.md` | specs S1–S8 and log; S6 = look, S7 = read-only keys. |

Model-picker control exception (S1): every row drawn by `model_section` maps to `Action::ModelPicker`; the dim Status `▸` is an additional button hint. Pure `act()` sets `Ui.open_picker`. Main consumes/clears it and sends a timestamp to `sources::model_picker`, a dedicated latest-request worker bound to the startup socket/pane. It discards triggers within 1 s of the last attempt, calls `pane.focus {pane_id}` then `pane.send_keys {pane_id, keys:["f9"]}` via `request()` (3 s read/write timeouts), and stops silently on failure. No control in `--once`, no arbitrary keys or alternate pane. Doctor permits exactly those two fixed builder entries.

Screenshot exception (`odd/tasks/gs3-flameshot-button.md`): `sources/screenshot.rs` runs one worker; `Action::Screenshot` sets `Ui.screenshot`, `main` forwards it, the worker spawns `flatpak run --branch=stable --arch=x86_64 --command=flameshot org.flameshot.Flameshot gui` detached (stdio null, own process group), ≥1 s apart.

Voice control exception (`odd/tasks/gs3-discord-voice.md`, S7–S9 supersede S4/S5): `gs3/hud/src/sources/voice.rs` runs one worker. Discovery tries `$XDG_RUNTIME_DIR/discord-ipc-0..9` and accepts a socket only when `/proc/net/unix` + `/proc/*/fd` show a `vesktop`/`vesktop.bin` process owns it (official Discord is unsupported by user decision) and it handshakes, authenticates and answers GET within 0.7 s; then SUBSCRIBE `VOICE_SETTINGS_UPDATE`, a reader thread, one SET in flight (2 s window; a timeout only frees the slot), rediscovery every 2 s while offline. The mic while deafened sends `{mute:false,deaf:false}` (Vesktop ignores a bare mute then) and is drawn red while deafened. AUTHORIZE only after a click, curl exchange, `save_token` (temp + rename, 0600). The gear spawns `vesktop` (PATH, else `~/.local/bin/vesktop`) at once, detached, rate-limited to 1/s: Electron single-instance focuses the running window.

Build/test: `cd gs3/hud && CARGO_TARGET_DIR=./target cargo test --release`. Binary: `./target/release/gs3-hud`. A running sidecar keeps the old binary: restart gs3 (or the HUD pane) after a build.

## Data flow

```
herdr thread ─┐  local thread (inotify JSONL, .git, refresh) ─┐  PR thread (gh) ─┐
              └──────────── mpsc<Event> ──────────────────────┴──────────────────┘
                                   │
                    main loop: apply(&mut State, Event) -> changed?
                                   │ yes (or resize)
                 ui::frame(&State,w,h) -> Frame{lines,hits} -> ratatui
                                   │ mouse: click(&last,col,row) / wheel() -> Action -> Event::Ui
```

- `apply()` returns `true` only when `State` changed: that is the only redraw trigger.
- `State.refresh` (set on turn end `idle`/`done` and key `r`) makes main send `Local::Refresh` and a PR trigger.
- `State.read_session` (new session path) makes main send `Local::Session(path)`.
- Clicks: a `Card` row gets `on_click(Action)`; the title row and its `[-]` always map to `Action::Collapse(card id)`; the narrowest hit wins. Hits are in screen rows (scroll and header applied).
- Add data = new field in `State` (or `Session`) + `Event` variant + `apply` arm + producer.

## Sources and triggers

| Data | Producer | Trigger |
| --- | --- | --- |
| pane, status, session path, workspace label | `herdr()`, `pane()` (`pane.get` + `workspace.get`) | `events.subscribe`; re-query on relevant event |
| tokens, cost, model, thinking, tool counts | `read_tail()` → `Tail::ingest` | inotify Modify on the session JSONL (appended bytes only) |
| new session after `/new` | `is_new_session_file()` → `pane()` | inotify Create of a sibling `.jsonl` (herdr sends no `pane_updated`) |
| context limit | `read_tail()` | once per model: `cpamc-models-cache.json`, then `models-store.json` in the agent dir |
| git | `git_snapshot()` | inotify `.git/HEAD`, `index`, `refs` (300 ms debounce) + refresh |
| engram | `engram()` | refresh (start, turn end, `r`) |
| MCP | `mcp()` | session change + refresh; sent only when it changed |
| PRs | `pr_cycle()` = `gh pr view` + `gh pr list` | start, `head_moved`, refresh; ≥30 s between cycles; idle 2→5→15 min |

Agent dir = session path before `/sessions/` (`agent_dir()`); for gs2 it is `~/.gentle-shell/agent`.

## herdr 0.9.5 socket (NDJSON `{"id","method","params"}`)

- `pane.get {pane_id}` → `.result.pane {pane_id, workspace_id, agent, agent_status, agent_session.value}`.
- `workspace.get {workspace_id}` → `.result.workspace.label`.
- `events.subscribe` with `pane.updated`, `pane.closed`, `pane.agent_status_changed` (needs `pane_id`), `workspace.focused`.
- Events on the wire: `pane_updated`, `pane_closed` (underscore) but `pane.agent_status_changed` (dot). `parse_wire` accepts both.
- Status values: `working`, `idle`, `blocked`, `done`, `unknown`.
- CLI for tests: `herdr pane get|list|read --lines N|send-text|send-keys <id> Enter|close`, `herdr workspace create --cwd --label --no-focus|close`.

## Pi session JSONL (one JSON per line)

- `{"type":"model_change","provider","modelId"}`; `{"type":"thinking_level_change","thinkingLevel"}`.
- `{"type":"message","message":{"role":"assistant","model","provider","usage":{input,output,cacheRead,cacheWrite,totalTokens,cost:{total}},"content":[{type:"text"|"thinking"|"toolCall",name,arguments}]}}`.
- `role:"toolResult"` messages carry tool output; `role:"user"` prompts.
- `totalTokens` of the last assistant message = current context used.
- Add a field: extend `Session` and the `"message"` (or new `type`) arm in `Tail::ingest`. It must stay incremental: no re-reading the file.

## Local files the sidecar may read (read-only)

`<agent>/mcp.json` (names + `command`/`url` only), `<agent>/models-store.json`, `<agent>/cpamc-models-cache.json`, `~/.engram/engram.db` (`observations` table, `project`, `deleted_at`). Never read `auth.json`, `mcp-auth.json`, `apiKey` values or `/root/mis-apis/*`.
