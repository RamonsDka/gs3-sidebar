# Message frames (local, experimental)

Presentation-only framing for the installed Pi 1.5.0 / Gentle Pi main build.
Install it by copying this directory to `~/.pi/agent/extensions/message-frames/`
(`install.sh` does it). Its `index.ts` is auto-discovered. No
settings changes or dependencies are needed. Run `/reload` to load it.

- `/message-frames on` installs the wrappers.
- `/message-frames off` restores the original render and mouse descriptors immediately;
  the next normal redraw shows the original presentation.
- `/message-frames status` reports the current state. Other arguments show
  `Usage: /message-frames on|off|status` without changing state.
- Shutdown/reload restores the originals; a new runtime defaults to on.
  To disable permanently, exclude this extension through the host's existing
  extension controls, or remove its directory from discovery while Pi is closed.

## Integration boundary

The actual chat-route prototypes are wrapped: UserMessageComponent,
AssistantMessageComponent, ToolExecutionComponent, BashExecutionComponent,
CustomMessageComponent (including registered message renderers),
CompactionSummaryMessageComponent, BranchSummaryMessageComponent,
SkillInvocationMessageComponent and CustomEntryComponent. The last class is not
exported, so a constructed isolated entry through the host's own chat route
captures its constructor without loading a duplicate package instance or reading
session data. `render` and existing `handleMouse` methods are wrapped; mouse
coordinates map back to original rows, removed chrome columns, content width and
height. New frame rails do not dispatch clicks to content.
No tools, message transformers, model
hooks, execution handlers, schema changes, session access or execution overrides
are registered. Original renderers still own Markdown, thinking, streaming,
collapse/expand state, truncation and content. Render width reserves two columns.
A process-wide symbol shares ownership across repeated installs; final cleanup
restores the exact previous render/mouse descriptors, including inherited methods.
Missing render APIs and competing wrappers at install time produce a visible
notification and leave framing disabled. This is private-version-coupled code,
not a promise of support for future Pi versions. Re-test before upgrading.

Rounded outline and Gentle float chrome are recognized only as complete outer
cards; their titles and body remain, but their rails are replaced. Manual Bash's
first/last native horizontal rules are replaced. Native backgrounds are replaced,
while foreground SGR (including RGB/indexed colors), attributes and OSC survive.
The new border is re-colored after content resets. Blank and right-padding rows
receive the same continuous background as text. Grain is a stable low-amplitude
row variation, combined with a subtle vertical gradient. **Transparency is an
ANSI truecolor simulation over a dark base, not real alpha compositing.**

## Verification

From the repository root, with `PI_PKG_ROOT` pointing to the installed `@earendil-works/pi-coding-agent` package (the tests print SKIP without it):

```sh
node --test extensions/message-frames/frame.test.mjs
node --test extensions/message-frames/integration.test.mjs
node --test extensions/message-frames/*.test.mjs
script -q -e -c 'node --test extensions/message-frames/integration.test.mjs' /dev/null
```

Tests use only isolated constructed messages and installed render classes, with
no providers, sessions or shell/tool executions. Integration also uses Pi's
actual extension loader and confirms prototype identity and restore/reload.
The tests load Pi's render classes from `PI_PKG_ROOT`. No TypeScript compiler was
available in the inspected installation; loader success is not a typecheck.
A human `/reload` visual check is the normal post-load acceptance step, not an
automated verification result. The PTY command checks isolated integration only.

## Known limitations / review focus

One-column terminals cannot show both corners or a title; narrow frames clip
content according to the installed TUI width helpers. Empty components that Pi
intentionally hides remain hidden, including custom entries whose renderer returns
nothing. All nine native message/custom-entry routes above are covered; native
system messages remain hidden. Summary and skill collapse/expand behavior stays
with Pi. Regression tests cover tool mouse dispatch for plain, outline and float
renderers, including original row/column/width/height and descriptor restoration.
Inline terminal images have not been visually verified.
Other extensions that replace render methods after installation cannot be safely
unwrapped; disabling does not overwrite another extension's later replacement.
The simulated fill is designed for dark terminals; it does not sample the actual
terminal background. No editor, footer, header, HUD or widgets are modified.

## Reference and license

Consulted raw GitHub sources in `j0k3r-dev-rgl/j0k3r-pi` on `main`:
`extensions/j0k3r-theme/src/render/borders.ts`,
`extensions/j0k3r-theme/src/render/ToolCard.ts`, and
`extensions/subagents/src/render/completion-message.ts`.
Only the rounded glyph vocabulary (`╭ ╮ ╰ ╯ │ ─`) and titled-role visual idea
were used. Implementation is original; no source or execution overrides copied.
The repository-root `LICENSE` raw URL returned HTTP 404; no upstream license is
asserted and no unlicensed code was transplanted.
