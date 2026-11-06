import { test } from 'node:test';
import assert from 'node:assert/strict';
import { createState, transition, install } from './core.mjs';

const model = { id: 'reasoner', provider: 'fixture', reasoning: true };
const capabilities = {
  getSupportedThinkingLevels: m => m.reasoning ? ['off', 'low', 'medium', 'high'] : ['off'],
  clampThinkingLevel: (m, level) => !m.reasoning ? 'off' : level === 'xhigh' ? 'high' : level,
};

test('model → effort → result only after effort confirmation', () => {
  const start = createState('medium');
  assert.deepEqual(start, { step: 'model', currentLevel: 'medium' });
  const effort = transition(start, { type: 'model', model }, capabilities);
  assert.deepEqual(effort, { step: 'effort', currentLevel: 'medium', model, levels: ['off', 'low', 'medium', 'high'], level: 'medium' });
  assert.deepEqual(transition(effort, { type: 'level', level: 'high' }, capabilities), {
    step: 'done', result: { model, level: 'high' },
  });
  assert.deepEqual(start, createState('medium'), 'transitions do not mutate their input');
});

test('Escape closes from both steps without a result', () => {
  const start = createState('high');
  const effort = transition(start, { type: 'model', model }, capabilities);
  assert.deepEqual(transition(effort, { type: 'cancel' }, capabilities), { step: 'done', result: undefined });
  assert.deepEqual(transition(start, { type: 'cancel' }, capabilities), { step: 'done', result: undefined });
});

test('non-reasoning model offers off only; unsupported current level is clamped', () => {
  const plain = transition(createState('high'), { type: 'model', model: { ...model, reasoning: false } }, capabilities);
  assert.deepEqual(plain.levels, ['off']);
  assert.equal(plain.level, 'off');
  const reasoning = transition(createState('xhigh'), { type: 'model', model }, capabilities);
  assert.equal(reasoning.level, 'high');
});

function harness(result, success = true) {
  const commands = new Map(), shortcuts = new Map(), calls = [];
  const pi = {
    registerCommand: (name, options) => commands.set(name, options),
    registerShortcut: (name, options) => shortcuts.set(name, options),
    getThinkingLevel: () => 'medium',
    getSettings: () => ({ defaultProvider: 'fixture', defaultModel: 'reasoner' }),
    setModel: async m => { calls.push(['model', m]); return success; },
    setThinkingLevel: level => calls.push(['level', level]),
  };
  const ctx = { mode: 'tui', hasUI: true, modelRegistry: { runtime: {} }, scopedModels: [],
    ui: { custom: async (_factory, options) => { calls.push(['custom', options]); return result; },
      notify: (...args) => calls.push(['notify', ...args]) } };
  install(pi, capabilities);
  return { commands, shortcuts, calls, ctx };
}

test('F9 and command share one overlay and apply model before effort without saving defaults', async () => {
  for (const shortcut of [false, true]) {
    const h = harness({ model, level: 'high' });
    const handler = shortcut ? h.shortcuts.get('f9').handler : ctx => h.commands.get('model-picker').handler('', ctx);
    await handler(h.ctx);
    assert.deepEqual(h.calls, [
      ['custom', { overlay: true, overlayOptions: { anchor: 'center', width: '45%', minWidth: 44, maxHeight: '85%' } }],
      ['model', model], ['level', 'high'], ['notify', 'Modelo: reasoner · esfuerzo: high', 'info'],
    ]);
    assert.match(h.commands.get('model-picker').description, /F9/);
  }
});

test('cancel leaves model and effort unchanged; failed model selection never sets effort', async () => {
  const cancelled = harness(undefined);
  await cancelled.shortcuts.get('f9').handler(cancelled.ctx);
  assert.equal(cancelled.calls.length, 1);
  const failed = harness({ model, level: 'high' }, false);
  await failed.shortcuts.get('f9').handler(failed.ctx);
  assert.deepEqual(failed.calls.slice(1), [['model', model], ['notify', 'No se pudo activar el modelo: reasoner', 'error']]);
});

test('RPC, print, JSON and missing UI never open an overlay or change selection', async () => {
  for (const [mode, hasUI] of [['rpc', true], ['print', false], ['json', false], ['tui', false]]) {
    const h = harness({ model, level: 'high' });
    await h.shortcuts.get('f9').handler({ ...h.ctx, mode, hasUI });
    assert.deepEqual(h.calls, [['notify', 'El selector de modelo requiere el modo interactivo TUI.', 'warning']]);
  }
});
