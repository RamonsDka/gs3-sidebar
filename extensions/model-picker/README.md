# Model picker

A centered, narrow popup framed with the active theme's accent border (`borderAccent`) for selecting a model and then its thinking effort in one interaction. Uses Pi 1.5.0's native model and thinking selectors, including model search, scope switching and catalog refresh.

- Press **F9** or run **`/model-picker`** in gs2 (Pi's interactive TUI).
- Choose a model with Enter, then choose an effort with Enter to apply both.
- Escape (on either step) or a click on the **✕** in the top border closes the popup without changing anything.
- Changes apply to the current session only; configured defaults are never changed. Default model badges come from the public `pi.getSettings()` API. Native thinking-selector help may mention saving defaults, but that action is deliberately not wired here.

The gs3 model button triggers this extension by focusing the bound gs2 pane with herdr `pane.focus`, then sending the fixed `f9` key with `pane.send_keys`. It does not type a prompt or execute a slash command. The HUD-side click handler is implemented separately.

Auto-discovered from this directory's `index.ts` in the gs2 agent extensions tree, like sibling extensions. Requires Pi 1.5.0: the native model selector needs the `ModelRuntime` exposed by the registry facade's JavaScript `runtime` field. Other run modes notify rather than opening a popup.

Run tests here with `node --test`. Integration tests use the installed Pi 1.5.0 store path, in-memory model fixtures and a render-request stub; they never load credentials or real provider catalogs.
