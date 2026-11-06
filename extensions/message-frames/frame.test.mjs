import { test } from 'node:test';
import assert from 'node:assert/strict';
import { createRequire } from 'node:module';
const root = process.env.PI_PKG_ROOT;
if (!root) { console.log('SKIP: define PI_PKG_ROOT (carpeta del paquete @earendil-works/pi-coding-agent) para correr este test'); process.exit(0); }
const require = createRequire(root + '/package.json');
export const tui = await import(require.resolve('@earendil-works/pi-tui'));
const { frameLines, installFrames } = await import('./frame.mjs');
export const plain = s => s.replace(/\x1b\[[0-9;:]*m/g, '').replace(/\x1b\][^\x07]*\x07/g, '');

test('rounded role frames: full geometry, ANSI, Unicode, empty and narrow rows', () => {
 for (const width of [0, 1, 2, 3, 4, 12, 60]) {
  for (const body of [[], [''], ['\x1b[31m界 é 👩‍💻\x1b[0m', '', 'tail'], ['a\nb']]) {
   const lines = frameLines(body, width, 'User', [90,170,230], tui);
   if (!width) { assert.deepEqual(lines, []); continue; }
   for (const line of lines) { assert.equal(tui.visibleWidth(line), width); assert.ok(line.endsWith('\x1b[0m')); assert.match(line, /\x1b\[48;2;/); }
   if (width >= 2) { assert.ok(plain(lines[0]).startsWith('╭')); assert.ok(plain(lines[0]).endsWith('╮')); assert.ok(plain(lines.at(-1)).startsWith('╰')); assert.ok(plain(lines.at(-1)).endsWith('╯')); }
   assert.deepEqual(lines, frameLines(body, width, 'User', [90,170,230], tui));
  }
 }
 const colored = frameLines(['\x1b[38;2;40;49;100mRGB\x1b[0m'],30,'User',[90,170,230],tui);
 assert.match(colored[1], /\x1b\[38;2;40;49;100mRGB/);
 const rows = frameLines(['\x1b[31mred\x1b[0m tail', ''], 30, 'Assistant', [90,170,230], tui);
 assert.match(rows[0], /Assistant/); assert.match(rows[1], /\x1b\[31mred/);
 assert.match(rows[1], /\x1b\[0m\x1b\[48;2;/);
 assert.match(rows[2], /\x1b\[48;2;/); assert.equal(plain(rows[2]), '│' + ' '.repeat(28) + '│');
 assert.notEqual(rows[1].match(/48;2;[^m]+/)[0], rows[2].match(/48;2;[^m]+/)[0]);
});

test('wrapper delegates, restores, handles reload and removes existing chrome only', () => {
 class Message { render(w) { return ['\x1b[32mhello\x1b[0m', '', 'x'.repeat(w)]; } update() { return 42; } }
 const original = Message.prototype.render, update = Message.prototype.update;
 const instance = new Message();
 const targets = [[Message, () => 'Tool: read', [90,170,230]]];
 const a = installFrames(targets, tui); const b = installFrames(targets, tui);
 assert.equal(Message.prototype.update, update); assert.equal(instance.update(), 42);
 const rows = instance.render(30); assert.equal(rows.length, 5); assert.match(plain(rows[0]), /Tool: read/);
 assert.deepEqual(rows.slice(1,-1).map(s => plain(s).slice(1,-1).trimEnd()), original.call(instance,28).map(s => plain(s).trimEnd()));
 a.restore(); assert.notEqual(Message.prototype.render, original);
 b.restore(); assert.equal(Message.prototype.render, original);
 const c = installFrames(targets,tui); c.restore(); c.restore(); assert.equal(Message.prototype.render,original);
 const chrome = frameLines(['╭─ old ──╮','│ body   │','╰────────╯'], 16, 'Subagent', [90,170,230], tui);
 assert.equal(chrome.map(plain).join('').split('╭').length - 1,1);
 assert.match(chrome.map(plain).join(''), /old/); assert.match(chrome.map(plain).join(''), /body/);
 assert.throws(() => installFrames([[class {}, ()=>'Bad', []]], tui), /Unsupported/);
});

test('outline slicing removes both original rails, including indented Unicode rows', () => {
 const rows=frameLines(['  ╭─ old ─╮','  │ 界 é  │','  ╰───────╯'],20,'Custom',[90,170,230],tui).map(plain);
 assert.equal(rows[2].split('│').length-1,2);
 assert.equal(rows[2].slice(1,-1).trim(),'界 é');
});
