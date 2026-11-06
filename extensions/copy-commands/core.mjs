// Only finalized fenced command blocks are offered; their contents are never executed.
export function extractCommands(text) {
  const commands = [];
  const fence = /^ {0,3}(`{3,}|~{3,})([^\n]*)\n([\s\S]*?)^ {0,3}\1[ \t]*$/gm;
  for (const match of text.matchAll(fence)) {
    const language = match[2].trim().toLowerCase();
    if (!['', 'bash', 'sh', 'shell', 'zsh', 'fish', 'console', 'powershell', 'pwsh', 'cmd', 'bat'].includes(language)) continue;
    const body = match[3].replace(/\n$/, '').replace(/^\$ /gm, '');
    if (body.trim()) commands.push(body);
  }
  return commands;
}

export function install(pi, { MouseRegion, truncateToWidth, copyToClipboard }) {
  let commands = [];
  const textOf = message => (message.content ?? []).filter(part => part.type === 'text').map(part => part.text).join('\n');
  const copy = async (index, ctx) => {
    if (!Number.isInteger(index) || !commands[index]) {
      ctx.ui.notify('No hay un comando con ese número.', 'warning');
      return;
    }
    const text = commands[index];
    try {
      await copyToClipboard(text);
      ctx.ui.notify(`Comando ${index + 1} copiado. No se ejecutó.`, 'info');
    } catch (error) {
      ctx.ui.notify(error instanceof Error ? error.message : String(error), 'error');
    }
  };
  const refresh = ctx => {
    if (ctx.mode !== 'tui') return;
    if (!commands.length) { ctx.ui.setWidget('copy-commands', undefined); return; }
    const snapshot = [...commands];
    ctx.ui.setWidget('copy-commands', (_tui, theme) => new MouseRegion({
      render(width) {
        return snapshot.map((command, index) => truncateToWidth(`${theme.fg('accent', '⧉')} ${command.replace(/[\r\n\t\x00-\x1f\x7f-\x9f]/g, ' ')}`, width));
      },
      invalidate() {},
    }, event => {
      if (event.button !== 'left' || event.y < 0 || event.y >= snapshot.length || event.x < 0 || event.x >= 1) return;
      if (event.type === 'click') { void copy(event.y, ctx); return { handled: true }; }
      if (event.type === 'press' || event.type === 'release') return { handled: true };
    }));
  };
  pi.on('session_start', (_event, ctx) => {
    const entries = ctx.sessionManager.getBranch();
    const last = [...entries].reverse().find(entry => entry.type === 'message' && entry.message.role === 'assistant');
    commands = last ? extractCommands(textOf(last.message)) : [];
    refresh(ctx);
  });
  pi.on('message_end', (event, ctx) => {
    if (event.message.role !== 'assistant') return;
    commands = extractCommands(textOf(event.message));
    refresh(ctx);
  });
  pi.registerCommand('copiar-comando', {
    description: 'Copiar un bloque de comando sin ejecutarlo: /copiar-comando [número]',
    handler: async (args, ctx) => {
      if (!ctx.hasUI) return;
      if (!commands.length) { ctx.ui.notify('La última respuesta no contiene bloques de comandos.', 'info'); return; }
      if (args.trim()) { await copy(/^\d+$/.test(args.trim()) ? Number(args.trim()) - 1 : -1, ctx); return; }
      if (commands.length === 1) { await copy(0, ctx); return; }
      const options = commands.map((command, index) => `${index + 1}. ${command.replace(/\s+/g, ' ').slice(0, 100)}`);
      const selected = await ctx.ui.select('Copiar comando (sin ejecutar)', options);
      if (selected !== undefined) await copy(options.indexOf(selected), ctx);
    },
  });
}
