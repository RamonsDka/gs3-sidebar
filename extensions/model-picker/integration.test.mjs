import { test } from 'node:test';
import assert from 'node:assert/strict';
import { createRequire } from 'node:module';
import { createPicker } from './core.mjs';
const root = process.env.PI_PKG_ROOT;
if (!root) { console.log('SKIP: define PI_PKG_ROOT (carpeta del paquete @earendil-works/pi-coding-agent) para correr este test'); process.exit(0); }
const require = createRequire(root + '/package.json');
const pi = await import(root + '/dist/index.js');
// pi-ai exposes import-only package conditions; resolve its installed sibling directly.
const ai = await import(root + '/../pi-ai/dist/index.js');
const tui = await import(require.resolve('@earendil-works/pi-tui'));
const deps = { ...pi, ...ai, truncateToWidth: tui.truncateToWidth, visibleWidth: tui.visibleWidth };
const models = [
  { id: 'reasoner', name: 'Reasoner', provider: 'fixture', reasoning: true },
  { id: 'plain', name: 'Plain', provider: 'fixture', reasoning: false },
];

const colors = new Set();
function fixture(done, overrides = {}) {
  let refreshSignal;
  const runtime = {
    getAvailableSnapshot: () => models,
    getModel: (provider, id) => models.find(m => m.provider === provider && m.id === id),
    getError: () => undefined,
    refresh: async ({ signal }) => { refreshSignal = signal; return { errors: new Map(), aborted: false }; },
  };
  const picker = createPicker({ tui: { requestRender() {} }, theme: { fg: (color, text) => { colors.add(color); return text; } },
    model: models[0], runtime, scopedModels: [], currentLevel: 'medium',
    defaultRef: { provider: 'fixture', id: 'reasoner' }, done, ...overrides }, deps);
  return { picker, signal: () => refreshSignal };
}
const rendered = picker => picker.render(60).join('\n');

test('installed Pi wrapper: list, effort, back, final result, disposal and focus', async () => {
  pi.initTheme('dark');
  const results = [];
  const { picker, signal } = fixture(value => results.push(value));
  picker.focused = true;
  assert.equal(picker.focused, true);
  assert.equal(picker.capturesViewportInput, true);
  assert.match(rendered(picker), /reasoner/);
  assert.match(rendered(picker), /plain/);
  assert.match(rendered(picker), /default/);
  assert.ok(picker.render(44).every(line => tui.visibleWidth(line) <= 44));
  picker.handleInput('\r');
  await new Promise(resolve => setImmediate(resolve));
  assert.equal(signal().aborted, true, 'leaving model selector releases its asynchronous refresh waiter');
  assert.match(rendered(picker), /Thinking Level/);
  assert.match(rendered(picker), /Esfuerzo · reasoner/);
  assert.deepEqual(results, []);
  picker.invalidate();
  picker.handleInput('\r');
  assert.deepEqual(results, [{ model: models[0], level: 'medium' }]);
  picker.dispose();
  await Promise.resolve();
});

test('installed Pi wrapper: model Escape cancels; non-reasoning clamps to off', async () => {
  pi.initTheme('dark');
  const cancelled = [];
  const first = fixture(value => cancelled.push(value));
  first.picker.handleInput('\x1b');
  assert.deepEqual(cancelled, [undefined]);
  await new Promise(resolve => setImmediate(resolve));
  assert.equal(first.signal().aborted, true);
  first.picker.dispose();
  const selected = [];
  const second = fixture(value => selected.push(value), { model: models[1], currentLevel: 'high' });
  second.picker.handleInput('\r');
  assert.match(rendered(second.picker), /Thinking Level/);
  second.picker.handleInput('\r');
  assert.deepEqual(selected, [{ model: models[1], level: 'off' }]);
  second.picker.dispose();
});

test('host helper capabilities include mapped xhigh/max, omit null and clamp upward then downward', () => {
  assert.deepEqual(ai.getSupportedThinkingLevels(models[1]), ['off']);
  assert.deepEqual(ai.getSupportedThinkingLevels(models[0]), ['off', 'minimal', 'low', 'medium', 'high']);
  const model = { ...models[0], thinkingLevelMap: { medium: null, xhigh: 'xhigh', max: 'max' } };
  assert.deepEqual(ai.getSupportedThinkingLevels(model), ['off', 'minimal', 'low', 'high', 'xhigh', 'max']);
  assert.equal(ai.clampThinkingLevel(model, 'medium'), 'high');
  assert.equal(ai.clampThinkingLevel(models[0], 'max'), 'high');
});

test('installed Pi loader discovers index.ts and registers F9 and /model-picker', async () => {
  const { loadExtensions } = await import(root + '/dist/core/extensions/loader.js');
  const loaded = await loadExtensions([new URL('./index.ts', import.meta.url).pathname], process.cwd());
  assert.deepEqual(loaded.errors, []);
  assert.ok(loaded.extensions[0].commands.has('model-picker'));
  assert.ok(loaded.extensions[0].shortcuts.has('f9'));
});

test('frame: rounded theme border on all sides, title, close button, exact width', () => {
  pi.initTheme('dark');
  colors.clear();
  const { picker } = fixture(() => {});
  for (const width of [44, 60]) {
    const lines = picker.render(width).map(line => line.replace(/\x1b\[[0-9;:]*m/g, ''));
    assert.ok(lines.every(line => tui.visibleWidth(line) === width), `width ${width}`);
    assert.match(lines[0], /^╭─ .*Modelo.* ✕ ─╮$/);
    assert.match(lines.at(-1), /^╰─.*Esc.*─╯$/);
    assert.ok(lines.slice(1, -1).every(line => line.startsWith('│ ') && line.endsWith(' │')));
  }
  assert.ok(colors.has('borderAccent'));
  picker.dispose();
});

test('close: clicking ✕ or Escape on the effort step closes without a result', async () => {
  pi.initTheme('dark');
  const closed = [];
  const { picker } = fixture(value => closed.push(value));
  const width = 60; const height = picker.render(width).length;
  const x = picker.render(width)[0].replace(/\x1b\[[0-9;:]*m/g, '').indexOf('✕');
  const event = type => ({ type, button: 'left', x, y: 0, screenX: x, screenY: 0, width, height, shift: false, alt: false, ctrl: false });
  assert.ok(picker.handleMouse(event('press'))?.handled);
  assert.deepEqual(closed, []);
  assert.ok(picker.handleMouse(event('click'))?.handled);
  assert.deepEqual(closed, [undefined]);
  picker.dispose();
  const second = fixture(value => closed.push(value));
  second.picker.handleInput('\r');
  assert.match(rendered(second.picker), /Thinking Level/);
  second.picker.handleInput('\x1b');
  assert.deepEqual(closed, [undefined, undefined]);
  second.picker.dispose();
  await new Promise(resolve => setImmediate(resolve));
});
