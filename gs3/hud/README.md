# gs3-hud

Read-only telemetry for one Pi pane. It follows the pane's `agent_session.value`
when Pi changes sessions, and exits when the bound pane closes.

```sh
gs3-hud --pane w1W:p1 [--socket /path/to/herdr.sock] [--cwd /path/to/repo] [--config /path/gs3-hud.json] [--once]
```

`--pane` is required. Socket defaults to `HERDR_SOCKET_PATH`, then
`~/.config/herdr/herdr.sock`. Working directory defaults to the current directory.
`--help` prints the CLI. `--once` prints every visible tab as plain text at width 42,
without raw mode or an alternate screen. Keys in the interactive UI: `1`–`6` switch tab,
`q` / Ctrl-C quit, `r` refreshes Git, Engram, MCP, and requests a PR refresh.
Mouse (button-only, no motion events): click a tab, a card's `[-]`/`[+]` (or its title)
to collapse/expand it, and CONFIG rows; the wheel over the tab bar cycles tabs and
elsewhere scrolls the body. MAIN's model section is a button: click any model-name,
effort or `sin modelo` row (or the dim Status `▸`) to open the model picker in gs2.
Above Status, three centered pixel-art icons (5×2, no frame) control Vesktop (≥ 1.6.9, official Discord is not supported) over its local RPC socket:
🎙 toggles mute, 🎧 toggles deafen (red = active, green = off, white = no client running), ⚙ opens or focuses Vesktop at once. Each whole icon (plus one column on each side) is clickable.
The first click asks for consent once in Vesktop; the token is kept private in `~/.config/gs3/discord-token.json` (0600).
It focuses only the bound pane, then sends fixed F9 to the model-picker extension.
A dedicated worker keeps socket I/O off rendering, coalesces pending clicks and
ignores triggers within 1 s of the last attempt; failures are silent. The extension
must be installed in gs2; the HUD does not type commands or try another pane.

`--config` is the HUD's own settings file `{name, hidden, collapsed}` (the CONFIG tab:
custom banner name, hidden tabs, collapsed cards). It is the only file the HUD writes,
via a temp file and rename; it never creates directories. Without `--config` changes
last until exit. `gs3` passes `~/.config/gs3/gs3-hud.json` (override with `GS3_CONFIG`).

Layout: the CUTE sidebar of pi-cinlodev-cute (banner `⌁ <name> ⌁` / `GENTLE-SHELL`, tab bar by name
`MAIN GIT PROF MEM TREE ⛭` (the purple gearbox `⛭` is CONFIG; banner and card titles use the purple power mark `⌁`; two-letter names when narrow),
violet double-framed collapsible cards). Card catalog: `skills/gs3-sidebar/references/cards.md`.

| Tab | Cards |
| --- | --- |
| 1:MAIN | voice buttons `🎙 🎧 ⚙` (Vesktop mute, deafen, open) plus a small Flameshot flame flush right (opens its area selector), ⌁ Status (project, branch, clickable model section: spaced capitals + `◆ THINKING ◆`; Status `▸` hint), ⌁ Context, ⌁ Todos (latest `todo` tool checklist of the session) |
| 2:GIT | ⎇ git graph (`[ Grafo \| Ramas ]`: commit graph or branches; click a branch to diff it against HEAD), 📁 Working Tree (changed files; click one for an inline diff preview, secret-like names are never read), ⌁ Pull requests |
| 3:PROF | ⌁ Profiles & Clusters (host model/thinking from `settings.json`, one row per `agents/*.md` with its model and effort, Task Manager footer) |
| 4:MEM | 🧠 Engram (daemon health, sync state), 🧠 Active Handoff (latest session summary, bounded), ⌁ Stack (versions probed once at start), 🔧 tools, ⌁ MCP (names only, never values) |
| 5:TREE | ⌁ PROYECTO (project tree from the git root, dirs first, git badges `M A ? U D`, `●` on folders with changes; click a folder to open/close it, `[ ⊟ ]` folds all, a changed file opens its diff in GIT; names only, symlinks never followed, build dirs skipped) |
| 6:CONFIG | ⌁ Nombre, ⌁ Pestañas (show/hide tabs), ⌁ Monitor (pane, config file, help) |

The context limit comes from `<agent dir>/cpamc-models-cache.json` (what Pi registers
at runtime), then `models-store.json`. Token counts are abbreviated like the reference (`33.7k`, `179k`, `1.0M`).

The terminal UI redraws only on changed state or resize. Herdr uses a dedicated
subscription socket and short-lived request sockets, reconnecting at 1–30 seconds.
Session append notifications update a byte-offset JSONL tail; Git notifications
are debounced by 300 ms. Engram refreshes at startup, turn end, and `r`. The TREE tab scans only while it is shown:
on entering it, on folder toggle, on `r`/turn end and on each Git change; a worker answers
only the newest request and stale scans are dropped.
Git subprocesses have a three-second deadline, `gh` eight seconds. Git optional
locks are disabled. SQLite is opened read-only; the only pane-control exception is the explicit model-picker click (focus + F9).

For GitHub origins, one PR cycle is two `gh` calls (current-branch PR, then open
count). Cycles run at start, on HEAD change, on agent turn end and on `r`; triggers
coalesce, and a cycle never starts less than 30 seconds after the previous one
(`r` queues rather than bypasses it). Idle cycles back off at 2, 5, then 15 minutes.
`--once` runs a single cycle.
Errors replace the affected data rather than retaining old data as fresh. The
open count is limited to 100 (`100+` means the limit was reached).

Build and test from this directory:

```sh
CARGO_TARGET_DIR=./target cargo test --release
CARGO_TARGET_DIR=./target cargo build --release
```

Binary: `./target/release/gs3-hud`. Dependencies include bundled
SQLite and no OpenSSL or HTTP client. The launcher should pass the **Pi pane's**
id, not the HUD pane's id. It can terminate the HUD pane after Pi exits.
