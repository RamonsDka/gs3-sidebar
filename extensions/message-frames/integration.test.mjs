import { test } from 'node:test';
import assert from 'node:assert/strict';
import { createRequire } from 'node:module';
import { installFrames } from './frame.mjs';
const root = process.env.PI_PKG_ROOT;
if (!root) { console.log('SKIP: define PI_PKG_ROOT (carpeta del paquete @earendil-works/pi-coding-agent) para correr este test'); process.exit(0); }
const require = createRequire(root+'/package.json');
const pi = await import(root+'/dist/index.js');
const tui = await import(require.resolve('@earendil-works/pi-tui'));
const { loadExtensions } = await import(root+'/dist/core/extensions/loader.js');
const plain = s => s.replace(/\x1b\[[0-9;:]*m/g,'').replace(/\x1b\][^\x07]*\x07/g,'');

test('installed Pi classes: isolated public render, streaming, restore, shared prototypes', async () => {
 // initTheme reads bundled theme assets only; no settings, sessions or providers.
 pi.initTheme('dark');
 const assistant = new pi.AssistantMessageComponent({role:'assistant',content:[{type:'thinking',thinking:'consider 界'},{type:'text',text:'answer é'}],stopReason:'stop'});
 const tool = new pi.ToolExecutionComponent('read','isolated',{path:'fixture'}, {showImages:false},undefined,{requestRender(){}},'/tmp');
 tool.updateResult({content:[{type:'text',text:'line one\n\nline two'}],details:{},isError:false});
 const bash = new pi.BashExecutionComponent('isolated display only',{requestRender(){}});
 bash.appendOutput('hello\nworld'); bash.setComplete(0,false);
 const cases = [[new pi.UserMessageComponent('hello 界\n\nworld'),'User'],[assistant,'Assistant'],[tool,'Tool: read'],[bash,'Bash'],[new pi.CustomMessageComponent({customType:'subagent-result',content:'done',display:true}),'Subagent']];
 for (const [component,title] of cases) {
  const Class = component.constructor;
  const original = Class.prototype.render;
  const originalUpdate = Class.prototype.invalidate;
  const baseline = component.render(38).map(plain);
  const control = installFrames([[Class,()=>title,[100,180,220]]],tui);
  for (const width of [4,20,40,80]) {
   const rows = component.render(width);
   assert.ok(rows.every(s=>tui.visibleWidth(s)===width));
   assert.equal(rows.filter(s=>plain(s).startsWith('╭')).length,1);
   assert.ok(plain(rows.at(-1)).endsWith('╯'));
  }
  if (title !== 'Bash') assert.deepEqual(component.render(40).slice(1,-1).map(s=>plain(s).slice(1,-1)),baseline.map(s=>s+' '.repeat(Math.max(0,38-tui.visibleWidth(s)))));
  assert.equal(Class.prototype.invalidate,originalUpdate);
  control.restore(); assert.equal(Class.prototype.render,original);
  assert.deepEqual(component.render(38).map(plain),baseline);
 }
 assistant.updateContent({role:'assistant',content:[{type:'text',text:'streaming now'}]},true);
 const c = installFrames([[pi.AssistantMessageComponent,()=> 'Assistant',[100,180,220]]],tui);
 assert.match(assistant.render(40).map(plain).join(''),/streaming now/); c.restore();
 const hidden = new pi.AssistantMessageComponent({content:[{type:'toolCall',id:'x',name:'read',arguments:{}}]});
 assert.deepEqual(hidden.render(40),[]);
});

test('all installed summary, skill and custom-entry chat routes are framed and restored', async () => {
 pi.initTheme('dark');
 const components = [new pi.CompactionSummaryMessageComponent({summary:'compact body',tokensBefore:123}),new pi.BranchSummaryMessageComponent({summary:'branch body'}),new pi.SkillInvocationMessageComponent({name:'fixture',content:'skill body'})];
 const makeEntry = renderer => {
  let component;
  pi.InteractiveMode.prototype.addCustomEntryToChat.call({session:{extensionRunner:{getEntryRenderer:()=>renderer}},outputPad:1,toolOutputExpanded:false,chatContainer:{addChild:c=>{component=c;}}},{customType:'fixture',data:{}});
  return component;
 };
 components.push(makeEntry(()=>new tui.Text('entry body',0,0)));
 const hidden = new components[3].constructor({customType:'hidden'},()=>undefined);
 const originals=components.map(c=>c.render);
 const loaded=await loadExtensions([new URL('./index.ts',import.meta.url).pathname],process.cwd());
 assert.deepEqual(loaded.errors,[]);
 const ext=loaded.extensions[0], ctx={mode:'tui',ui:{notify(){}}};
 await ext.handlers.get('session_start')[0]({},ctx);
 try {
  for (const [i,c] of components.entries()) {
   const rows=c.render(40).map(plain);
   assert.ok(rows[0].startsWith('╭'),`route ${i} must be framed`);
   assert.ok(rows[0].includes(['Compaction','Branch','Skill','Custom entry'][i]));
   assert.ok(rows.at(-1).endsWith('╯')); assert.ok(rows.every(s=>tui.visibleWidth(s)===40));
   c.setExpanded(true); assert.ok(c.render(40).map(plain).join('').includes(['compact body','branch body','skill body','entry body'][i]));
  }
  assert.deepEqual(hidden.render(40),[]); assert.equal(hidden.hasContent(),false);
 } finally { await ext.handlers.get('session_shutdown')[0]({},ctx); }
 components.forEach((c,i)=>assert.equal(c.render,originals[i]));
});

test('native self-render tool mouse maps plain, outline and float rows and restores ownership', () => {
 pi.initTheme('dark');
 const proto=pi.ToolExecutionComponent.prototype, original=proto.handleMouse;
 const hue=[100,180,220];
 for (const style of ['plain','outline','float']) {
  const text=style==='plain'?'one\ntwo\nthree':style==='outline'?'╭─ old ─╮\n│ one   │\n│ two   │\n│ three │\n╰───────╯':'▎ old\n▎ one\n▎ two\n▎ three\n▎';
  let received;
  const call=new tui.Text(text,0,0); call.handleMouse=event=>{received=event;};
  const tool=new pi.ToolExecutionComponent('agent','isolated',{}, {showImages:false},{renderShell:'self',renderCall:()=>call,renderResult:()=>new tui.Text('',0,0)},{requestRender(){}},'/tmp');
  tool.updateResult({content:[{type:'text',text:'ok'}],details:{}});
  tool.render(40); tool.handleMouse({type:'click',button:'left',x:3,y:3,width:40,height:tool.render(40).length});
  assert.equal(tool.expanded,true); tool.setExpanded(false);
  const a=installFrames([[pi.ToolExecutionComponent,()=> 'Tool: agent',hue]],tui);
  const b=installFrames([[pi.ToolExecutionComponent,()=> 'Tool: agent',hue]],tui);
  try {
   const rows=tool.render(40).map(plain);
   const y=style==='plain'?4:rows.findIndex(s=>s.includes('three'));
   tool.handleMouse({type:'click',button:'left',x:3,y,width:40,height:rows.length});
   assert.equal(tool.expanded,true,style);
   assert.equal(received.width,38); assert.equal(received.x,style==='plain'?2:3);
   assert.equal(received.y,style==='plain'?2:3);
   assert.equal(received.height,text.split('\n').length);
   tool.setExpanded(false); tool.render(40);
   tool.handleMouse({type:'click',button:'left',x:0,y,width:40,height:rows.length}); assert.equal(tool.expanded,false,'frame rail must not click content');
   a.restore(); assert.notEqual(proto.handleMouse,original);
  } finally {a.restore(); b.restore();}
  assert.equal(proto.handleMouse,original);
 }
});

test('actual Pi loader, lifecycle, command help and reload identity', async () => {
 const loaded = await loadExtensions([new URL('./index.ts',import.meta.url).pathname],process.cwd());
 assert.deepEqual(loaded.errors,[]);
 const ext = loaded.extensions[0];
 assert.equal(ext.tools.size,0); assert.equal(ext.messageRenderers.size,0);
 assert.match(ext.commands.get('message-frames').description,/on \| off \| status/);
 const notices=[]; const ctx={mode:'tui',ui:{notify:(...v)=>notices.push(v)}};
 const original = pi.UserMessageComponent.prototype.render;
 for (let i=0;i<2;i++) {
  await ext.handlers.get('session_start')[0]({},ctx);
  assert.notEqual(pi.UserMessageComponent.prototype.render,original);
  await ext.handlers.get('session_shutdown')[0]({},ctx);
  assert.equal(pi.UserMessageComponent.prototype.render,original);
 }
 const command=ext.commands.get('message-frames').handler;
 await command('off',ctx); await command('on',ctx); assert.notEqual(pi.UserMessageComponent.prototype.render,original);
 await command('off',ctx); assert.equal(pi.UserMessageComponent.prototype.render,original);
 await command('invalid',ctx); assert.deepEqual(notices.at(-1),['Usage: /message-frames on|off|status','error']);
 await command('status',ctx); assert.deepEqual(notices.at(-1),['Message frames: off','info']);
});

test('OSC 133 prompt zones stay at line start where the host filters them', () => {
 pi.initTheme('dark');
 const prefix = /^(?:\x1b\]133;[ABC](?:\x07|\x1b\\))+/;
 const cases = [[pi.UserMessageComponent,new pi.UserMessageComponent('hello'),'User'],[pi.AssistantMessageComponent,new pi.AssistantMessageComponent({role:'assistant',content:[{type:'text',text:'answer'}],stopReason:'stop'}),'Assistant']];
 for (const [Class,component,title] of cases) {
  const zones = s => s.match(/\x1b\]133;[ABC]\x07/g) ?? [];
  const baseline = component.render(38);
  const control = installFrames([[Class,()=>title,[100,180,220]]],tui);
  try {
   const rows = component.render(40);
   assert.deepEqual(zones(rows.join('')),zones(baseline.join('')),'markers preserved in order');
   assert.match(rows[0],/^\x1b\]133;A\x07/,'prompt start marks the frame top');
   for (const line of rows) assert.doesNotMatch(line.replace(prefix,''),/\x1b\]133;/,'no marker leaks past host filter');
  } finally { control.restore(); }
 }
});

test('OSC 133 final zones follow surviving native rows after unframing', () => {
 pi.initTheme('dark');
 const component = new pi.UserMessageComponent('╭─ old ─╮  \n│ body  │  \n╰───────╯');
 const native = component.render(38);
 assert.equal(native.length, 5);
 const control = installFrames([[pi.UserMessageComponent, () => 'User', [100,180,220]]], tui);
 try {
  const rows = component.render(40);
  assert.equal(rows.length, 6);
  assert.match(rows[0], /^\x1b\]133;A\x07/);
  assert.match(rows.at(-2), /^\x1b\]133;B\x07\x1b\]133;C\x07/);
  assert.doesNotMatch(rows.at(-1), /\x1b\]133;/);
  const prefix = /^(?:\x1b\]133;[ABC](?:\x07|\x1b\\))+/;
  for (const row of rows) assert.doesNotMatch(row.replace(prefix, ''), /\x1b\]133;/);
 } finally { control.restore(); }
});
