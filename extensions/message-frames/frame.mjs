const RESET = '\x1b[0m';
const SLOT = Symbol.for('local.message-frames.render.v1');
const sgr = /\x1b\[([0-9;:]*)m/g;
const plain = s => s.replace(sgr, '').replace(/\x1b\][^\x07]*\x07/g, '');
// Pi filters OSC 133 prompt zones only at line start; keep them there after framing.
const ZONE = /^(?:\x1b\]133;[ABC](?:\x07|\x1b\\))+/;

// Replace only background SGR parameters; foreground, attributes and OSC survive.
function background(text, bg) {
 return text.replace(sgr, (seq, params) => {
  const parts = params.split(';');
  const kept = [];
  let rearm = false;
  for (let i = 0; i < parts.length; i++) {
   const n = Number(parts[i].split(':')[0]);
   if (n === 38 || n === 58) {
    const count = parts[i].includes(':') ? 0 : parts[i+1] === '2' ? 4 : parts[i+1] === '5' ? 2 : 0;
    kept.push(...parts.slice(i,i+count+1)); i += count;
   } else if (n === 48) {
    rearm = true;
    if (!parts[i].includes(':')) i += parts[i+1] === '2' ? 4 : parts[i+1] === '5' ? 2 : 0;
   } else if (n === 49 || n >= 40 && n <= 47 || n >= 100 && n <= 107) rearm = true;
   else { kept.push(parts[i]); if (n === 0) rearm = true; }
  }
  return (kept.length ? `\x1b[${kept.join(';')}m` : '') + (rearm ? bg : '');
 });
}

function unframe(rows, tui) {
 const lines = rows.map(row => row.text);
 // Recognize a complete outer card, not arbitrary box characters in prose.
 const first = lines.findIndex(s => plain(s).trim().length);
 let last = lines.length - 1;
 while (last >= 0 && !plain(lines[last]).trim()) last--;
 const head = plain(lines[first] ?? '').trim();
 const tail = plain(lines[last] ?? '').trim();
 if (/^╭.*╮$/.test(head) && /^╰─*╯$/.test(tail) &&
     lines.slice(first+1,last).every(s => /^│.*│$/.test(plain(s).trim()))) {
  return rows.flatMap((row,i) => {
   const s = row.text;
   if (i === last) return [];
   if (i === first) {
    const lead = plain(s).match(/^\s*╭─* ?/)[0];
    return [{...row,x:tui.visibleWidth(lead),text:s.replace(/╭─* ?/, '').replace(/ *─*╮/, '')}];
   }
   if (i > first && i < last) {
    const p = plain(s), left = tui.visibleWidth(p.slice(0,p.indexOf('│'))), right = tui.visibleWidth(p.slice(0,p.lastIndexOf('│')));
    return [{...row,x:left+1,text:tui.sliceByColumn(s, left+1, right-left-1)}];
   }
   return [row];
  });
 }
 // Gentle float cards have a left accent rail on every non-margin row.
 if (first >= 0 && lines.slice(first,last+1).every(s => /^\s*▎/.test(plain(s)))) {
  return rows.map(row => {
   const col = plain(row.text).indexOf('▎');
   return col < 0 ? row : {...row,x:col+1,text:tui.sliceByColumn(row.text,col+1,tui.visibleWidth(row.text)-col-1)};
  });
 }
 return rows;
}

export function frameLines(input, width, title, hue, tui) {
 return compose(input.flatMap((s,y) => s.split('\n').map(text => ({text,y,x:0}))),width,title,hue,tui).lines;
}

function compose(input, width, title, hue, tui) {
 width = Math.max(0, Math.floor(width));
 if (!width) return {lines:[],map:[]};
 const inner = Math.max(0,width-2);
 const zones = input.map(row => row.text.match(ZONE)?.[0] ?? '');
 // Tag source indices so zones follow surviving rows when unframe drops chrome.
 const rows = unframe(input.map((row,src) => ({...row,src,text:row.text.replace(ZONE,'')})),tui);
 const lines = rows.map(row => row.text);
 const label = tui.truncateToWidth(` ${title} `, inner, '');
 const raw = [width === 1 ? '╭' : `╭${label}${'─'.repeat(Math.max(0,inner-tui.visibleWidth(label)))}╮`,
  ...(lines.length ? lines : ['']).map(s => {
   const fit = tui.truncateToWidth(s,inner,'');
   return width === 1 ? '│' : `│${fit}${' '.repeat(Math.max(0,inner-tui.visibleWidth(fit)))}│`;
  }), width === 1 ? '╰' : `╰${'─'.repeat(inner)}╯`];
 const paintedLines = raw.map((s,y) => {
  // Vertical wash plus stable low-amplitude grain; simulated alpha, no timers.
  const mix = 0.13 + 0.035*y/Math.max(1,raw.length-1) + ((y*17%7)-3)*0.002;
  const rgb = hue.map(v => Math.round(16*(1-mix)+v*mix));
  const bg = `\x1b[48;2;${rgb.join(';')}m`;
  const fg = `\x1b[38;2;${hue.join(';')}m`;
  const painted = background(s,bg);
  if (y > 0 && y < raw.length-1 && width > 1) {
   return bg + fg + '│\x1b[39m' + painted.slice(1,-1) + fg + '│' + RESET;
  }
  return bg + fg + painted + RESET;
 });
 // First zone (prompt start) marks the frame top; others keep their body row.
 zones.forEach((zone,i) => {
  if (!zone) return;
  let y = 0;
  if (i > 0) { const at = rows.findLastIndex(row => row.src <= i); y = at < 0 ? 0 : at+1; }
  paintedLines[y] = zone + paintedLines[y];
 });
 return {lines:paintedLines,map:[undefined,...rows.map(({src,...row}) => row),undefined]};
}

export function installFrames(targets, tui) {
 // Preflight all targets before touching any shared prototype.
 for (const [Class] of targets) if (typeof Class?.prototype?.render !== 'function') throw new Error('Unsupported Pi message component render API');
 const entries = targets.map(([Class,title,hue]) => {
  const proto = Class.prototype;
  const old = proto[SLOT];
  if (old && (proto.render !== old.wrapper || proto.handleMouse !== old.mouseWrapper)) throw new Error('Unsupported render wrapper conflict; reload without message-frames');
  return {proto,title,hue,old};
 });
 const owned = [];
 for (const {proto,title,hue,old} of entries) {
  if (old) { old.leases++; owned.push([proto,old]); continue; }
  const descriptor = Object.getOwnPropertyDescriptor(proto,'render');
  const original = proto.render;
  const mouseDescriptor = Object.getOwnPropertyDescriptor(proto,'handleMouse');
  const mouse = proto.handleMouse;
  const layouts = new WeakMap();
  const state = {original,descriptor,mouseDescriptor,mouse,leases:1,wrapper:undefined,mouseWrapper:mouse};
  state.wrapper = function(width) {
   layouts.delete(this);
   if (width <= 0) return [];
   const body = original.call(this,Math.max(1,width-2));
   if (!body.length) return body; // Preserve intentionally hidden/tool-only components.
   let rows = body.flatMap((s,y) => s.split('\n').map(text => ({text,y,x:0})));
   if (title(this) === 'Bash') {
    const indices = rows.flatMap((row,i) => /^─+$/.test(plain(row.text).trim()) ? [i] : []);
    if (indices.length >= 2) rows = rows.filter((_row,i) => i !== indices[0] && i !== indices.at(-1));
   }
   const framed = compose(rows,width,title(this),hue,tui);
   layouts.set(this,{map:framed.map,width:Math.max(1,width-2),height:body.length});
   return framed.lines;
  };
  if (typeof mouse === 'function') state.mouseWrapper = function(event) {
   const layout = layouts.get(this), row = layout?.map[event.y];
   if (!row || event.x < 1 || event.x >= layout.width+1) return undefined;
   return mouse.call(this,{...event,x:event.x-1+row.x,y:row.y,width:layout.width,height:layout.height});
  };
  Object.defineProperty(proto,SLOT,{value:state,configurable:true});
  Object.defineProperty(proto,'render',{value:state.wrapper,writable:true,configurable:true});
  if (typeof mouse === 'function') Object.defineProperty(proto,'handleMouse',{value:state.mouseWrapper,writable:true,configurable:true});
  owned.push([proto,state]);
 }
 let restored = false;
 return { restore() {
  if (restored) return;
  restored = true;
  for (const [proto,state] of owned) {
   if (--state.leases) continue;
   if (proto.render === state.wrapper) {
    if (state.descriptor) Object.defineProperty(proto,'render',state.descriptor);
    else delete proto.render;
   }
   if (proto.handleMouse === state.mouseWrapper && typeof state.mouse === 'function') {
    if (state.mouseDescriptor) Object.defineProperty(proto,'handleMouse',state.mouseDescriptor);
    else delete proto.handleMouse;
   }
   delete proto[SLOT];
  }
 } };
}
