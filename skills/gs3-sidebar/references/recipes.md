# Recipes

Paths are relative to `<REPO>`. Every recipe ends with `scripts/doctor.sh` and the Learn step from SKILL.md.

## Card: add, remove, reorder

1. RED test in `gs3/hud/src/core.rs` `mod tests` (extend `reference_layout_cards` or add one):
   ```rust
   let lines=render_lines(&s,43);
   find(&lines,"Mi tarjeta · 3");              // title present
   assert!(find(&lines,"Clave").contains("valor"));
   ```
   For removal assert `!lines.iter().any(|l|l.contains("Título"))`. The test also checks every card line is exactly the width and framed by `╭╮│╰╯`.
2. In `render()`, cards are blocks pushed in screen order:
   ```rust
   out.push(top(vec![seg("Mi tarjeta",Gold),seg(" · 3",Muted)],w)); // title
   out.push(keyed("Clave",vec![seg(valor,Text)],w));             // label kept whole, value cut
   out.push(row(vec![seg("izq",Text)],vec![seg("der",Dim)],w));  // right side wins
   out.push(bottom(w));
   ```
   Reorder = move the whole block. Remove = delete the block and any data/source that only it used (fields, `Event`, producer, test).
3. Update `references/cards.md` (heading `## Mi tarjeta` in the same position) and the layout table in `gs3/hud/README.md`.

## JSONL field (data already in the Pi session)

1. RED test like `tool_calls_are_counted_by_kind`: build JSON lines with `serde_json::json!`, `Tail::ingest(bytes,false)`, assert the new `Session` field.
2. Add the field to `Session` (derives `Default`), parse it in `Tail::ingest`. Aggregate incrementally (`+=`) or keep the latest value.
3. Render it. It refreshes for free (the JSONL tail already sends `Event::Session`).
4. Check the real format first: `python3 -c` over the newest `~/.gentle-shell/agent/sessions/*/*.jsonl` and print only types/keys, never contents that can hold secrets.

## New source (file, command, herdr)

1. Type in `core.rs` (`#[derive(Clone, Debug, Default, PartialEq)]`) and a pure parser fn + test (example: `mcp_servers(&Value)`).
2. `State` field + init in `State::new` + `Event::<Name>(T)` + `apply` arm (`Event::X(v)=>s.x=v`).
3. Producer in `sources.rs`:
   - file → read with `fs::read`, parse, send. Hook it on `Local::Refresh` (and `Local::Session` if it depends on the agent dir). Send only on change if it is noisy (see `send_mcp`).
   - command → `run(cwd,"prog",&args,secs)`, always with a timeout. Never in the render path. Rate-limit like PRs (`pr_due`, `pr_trigger_due`) if it is network or slow.
   - watched file → `w.watch(path,…)` in `local()` and a branch in `Local::Changed`. Debounce like git when it fires in bursts.
4. Fill it in `--once` (`main.rs`) so the snapshot shows it.
5. Errors: put the error in the data (`error:Option<String>`) and render it coral. Never keep stale data as if fresh.
6. Read-only: open DBs with `SQLITE_OPEN_READ_ONLY`. Show names, never secret values.

## Style

- Palette `Tone` (`core.rs`, RGB in `Tone::rgb`): Border violet, Gold labels, Mint ok, Amber warn, Coral error, Text, Muted, Dim, Cyan, Accent periwinkle, Pink. New color = new variant + rgb + use.
- UI copy is Spanish (rioplatense), code and docs English.
- Width: `--once` renders at 42; the real pane is ~25 % of the terminal. Test 43 and also narrow widths (`unicode_render` checks 0–42).
- Numbers: `fmt_tokens` (`950`, `179.2k`, `1.0M`). Paths: `display_path`/`short_path`.
- Thresholds/labels such as `ctx_band` live in pure fns with tests.

## Keys

S7: keys only `q`, Ctrl-C, `r`. A new key must be read-only (e.g. toggles, scroll). Destructive actions are out of scope unless the user explicitly asks and accepts a confirmation step. Update `HELP` in `main.rs`, the footer line in `render()`, and both READMEs.

## Launcher

`gs3/bin/gs3` (bash). Check with `bash -n gs3/bin/gs3`. Ratio/env overrides are documented in its header and `gs3/README.md`. HUD lookup: `$GS3_HUD`, `target/release/gs3-hud`, PATH.

## Live check

`scripts/live-test.sh "Leé gs3/README.md y resumilo en una línea. No edites nada."` creates a throwaway herdr workspace with real gs2 + HUD, sends the prompt, waits for idle, prints the HUD, CPU ticks and RSS, then closes the workspace. Compare the HUD with gs2's own footer (`herdr pane read <gs2 pane> --lines 12`).
