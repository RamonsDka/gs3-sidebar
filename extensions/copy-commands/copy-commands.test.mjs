import { test } from 'node:test';
import assert from 'node:assert/strict';
import { extractCommands, install } from './core.mjs';

test('extracts shell blocks without fences or prompt markers, preserving multiline syntax', () => {
  assert.deepEqual(extractCommands('```bash\n$ echo "hola"\n```\n```sh\nprintf "%s" \\\n  "dos"\n```\n```json\n{}\n```'), ['echo "hola"', 'printf "%s" \\\n  "dos"']);
  assert.deepEqual(extractCommands('```bash\necho unfinished'), []);
  assert.deepEqual(extractCommands('```\necho plain\n```'), ['echo plain']);
});

test('click and command copy exact text, cancellation and failures are safe', async () => {
  const events = {}, commands = {}, copied = [], notices = [];
  let widget, selected;
  const pi = { on: (name, fn) => events[name] = fn, registerCommand: (name, value) => commands[name] = value };
  const ui = { setWidget: (_key, value) => widget = value, notify: (...args) => notices.push(args), select: async () => selected };
  const ctx = { mode: 'tui', hasUI: true, ui, sessionManager: { getBranch: () => [] } };
  class MouseRegion { constructor(child, handler) { this.child = child; this.handleMouse = handler; } render(w) { return this.child.render(w); } }
  install(pi, { MouseRegion, truncateToWidth: (s, w) => s.slice(0,w), copyToClipboard: async text => { if (text === 'fail') throw Error('clipboard failed'); copied.push(text); } });
  await events.session_start({}, ctx);
  assert.equal(widget, undefined);
  const message = { role: 'assistant', content: [{type: 'text', text: '```bash\n$ echo hola\n```'}] };
  await events.message_end({ message }, ctx);
  const component = widget({}, { fg: (_color, text) => text });
  assert.equal(component.render(80)[0], '⧉ echo hola');
  assert.ok(component.render(8).every(line => line.length <= 8));
  assert.equal(component.handleMouse({ type: 'click', button: 'right', x: 0, y: 0 }), undefined);
  assert.equal(component.handleMouse({ type: 'click', button: 'left', x: 1, y: 0 }), undefined);
  component.handleMouse({ type: 'click', button: 'left', x: 0, y: 0 });
  await new Promise(resolve => setImmediate(resolve));
  assert.deepEqual(copied, ['echo hola']);
  await commands['copiar-comando'].handler('', ctx);
  assert.deepEqual(copied, ['echo hola', 'echo hola']);
  await events.message_end({message: {role:'assistant',content:[{type:'text',text:'```sh\na\n```\n```sh\nb\n```'}]}}, ctx);
  await commands['copiar-comando'].handler('', ctx);
  assert.equal(copied.length, 2);
  selected = '2. b';
  await commands['copiar-comando'].handler('', ctx);
  assert.equal(copied.at(-1), 'b');
  await commands['copiar-comando'].handler('99', ctx);
  assert.equal(copied.length, 3);
  await events.message_end({message: {role:'assistant',content:[{type:'text',text:'```sh\nfail\n```'}]}}, ctx);
  await commands['copiar-comando'].handler('',ctx);
  assert.deepEqual(notices.at(-1), ['clipboard failed','error']);
  await events.message_end({message:{role:'assistant',content:[{type:'text',text:'No commands'}]}},ctx);
  assert.equal(widget,undefined);
});
