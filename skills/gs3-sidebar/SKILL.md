---
name: gs3-sidebar
description: "Trigger: gs3, gs3-hud, barra lateral gs3, terminal lateral, sidecar de telemetría, tarjeta del HUD, agregar/quitar/modificar tarjeta, colores de la barra, gs3 sidebar. Change the gs3 read-only telemetry sidecar (Rust, herdr pane next to gs2) safely, and record every new learning."
license: MIT
metadata:
  author: RamonsDka
  version: "1.0"
---

## Activation Contract

Use for any request to add, remove, reorder, restyle, fix, or explain what the gs3 sidecar (`gs3-hud`) or its launcher (`gs3`) shows. Not for Pi's own footer, gs2 install, or herdr itself.

Repo: `<REPO>` (resolve via `readlink -f` of this skill if moved). Crate: `gs3/hud`. Skill dir: `$S` (this directory).

## Hard Rules

- Read first: `$S/references/lessons.md`, then `mem_search "gs3-sidebar"` (project `gs3-sidebar`). Past gotchas live there.
- Read-only telemetry sidecar: never write files/DBs or send text/keys to panes except these bounded exceptions. Explicit click on the MAIN model section (or Status `▸`) may focus the bound gs2 pane and send only fixed `f9`, opening its model-picker extension; one off-render worker, ≥1 s between triggers, silent failure, no fallback pane or other keys. File exception: `config.rs` `save()` writes the HUD's own `--config` file (`{name, hidden, collapsed}`, temp + rename); the doctor allows only that line. Voice exception (user-approved 2026-11-06, `odd/tasks/gs3-discord-voice.md`): `sources/voice.rs` talks to the local Discord/Vesktop RPC socket (`$XDG_RUNTIME_DIR/discord-ipc-N`) to read/subscribe mute/deaf state and, only on an explicit 🎙/🎧 click, send `SET_VOICE_SETTINGS`; it may store the Discord RPC token only in `~/.config/gs3/discord-token.json` (0600, temp + rename, never displayed or logged), and runs curl to streamkit only during the one-time AUTHORIZE triggered by a click. ⚙ click may launch the connected client's executable (Electron focuses its window), ≥1 s apart. Doctor allows exactly the tagged lines `// voice-token-save`, `// voice-token-rename`, `// voice-gear-spawn`. Voice exception (user-approved 2026-11-06, `odd/tasks/gs3-discord-voice.md`): `sources/voice.rs` talks to the local Discord/Vesktop RPC socket (`$XDG_RUNTIME_DIR/discord-ipc-N`) to read/subscribe mute/deaf and, only on an explicit 🎙/🎧 click, send `SET_VOICE_SETTINGS`; it may store the Discord RPC token only in `~/.config/gs3/discord-token.json` (0600, temp + rename, never displayed or logged) and runs curl to streamkit only during the one-time AUTHORIZE a click triggers. A ⚙ click may launch the connected client's own executable (Electron focuses its window), ≥1 s apart. Doctor allows exactly the tagged lines `// voice-token-save`, `// voice-token-rename`, `// voice-gear-spawn`. Screenshot exception (user-approved 2026-11-06, `odd/tasks/gs3-flameshot-button.md`): `sources/screenshot.rs` may run `flatpak run … org.flameshot.Flameshot gui` only on an explicit click of the flame icon on the MAIN voice rows, detached, no output read, ≥1 s apart; doctor allows exactly the tagged line `// screenshot-spawn`. Keys: `1`–`6` tabs, `r`, `q`/Ctrl-C, plus typing in the CONFIG name editor. Mouse is button-only (`?1000h` + SGR `?1006h`, never `?1003h`) and navigation-only except the explicit model-picker and voice actions: clicks never write repo files or launch other processes. A destructive action needs the user's explicit OK and a confirmation key.
- Event-driven only: no polling or redraw loop. External commands run off the render path with a timeout and rate limit (`run()`).
- Never show secrets or config values (auth.json, apiKey, headers, args). Names only.
- Width-safe text: build lines with `seg/keyed/row/top/bottom`; never byte-slice strings.
- Test-first: RED test through `render_lines`/`apply`, then code. Live tests only against gs2 (`~/.gentle-shell`); never touch `~/.gentle-shell`.
- No new crates, system packages, clippy or rustfmt without asking. No OpenSSL.

## Decision Gates

| Request | Read |
| --- | --- |
| Add/remove/reorder/restyle a card or row | `references/cards.md`, `references/recipes.md` §Card |
| New data from the Pi session | `references/architecture.md` §Pi JSONL, recipes §JSONL field |
| New file/command/herdr data | architecture §Sources, recipes §New source |
| Colors, labels, language, width | recipes §Style |
| Launcher, split ratio, pane pairing | `gs3/bin/gs3`, recipes §Launcher |
| Wrong numbers vs Pi | lessons.md first |

## Execution Steps

1. Read lessons, the card catalog, and the files you will change.
2. Write the RED test (`core.rs` tests; helper `find(&lines,"text")`). Run it and watch it fail.
3. Implement (recipes). If you add a source, also fill it in the `--once` branch of `main.rs`.
4. Run `$S/scripts/doctor.sh` (tests, build, render, catalog sync, read-only, width). It must pass.
5. For visible or live behavior: `$S/scripts/live-test.sh ["prompt"]` (real gs2 in a throwaway herdr workspace).
6. **Learn** (mandatory, same commit as the code):
   - update `references/cards.md` for any card change (doctor fails otherwise);
   - append to `references/lessons.md` every non-obvious finding (cause → fix → test);
   - update `architecture.md`/`recipes.md` when a pattern or source is new;
   - `mem_save` type `discovery`/`bugfix`, project `gs3-sidebar`, topic_key `gs3-sidebar/<slug>`.
7. Update `gs3/hud/README.md` (layout table) and `gs3/README.md` (refresh table) when behavior changes. Commit on the current feature branch.

## Output Contract

Return: what changed on screen, files touched, RED→GREEN evidence, doctor result, live-test output (or why skipped), learnings recorded (file + Engram id), risks.

## References

- `references/architecture.md`: files, State/Event flow, sources, triggers, herdr and Pi formats.
- `references/cards.md`: card catalog (doctor-checked).
- `references/recipes.md`: step-by-step changes.
- `references/lessons.md`: self-learning log.
- `scripts/doctor.sh`, `scripts/live-test.sh`.
