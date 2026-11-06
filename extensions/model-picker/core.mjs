export function createState(currentLevel) {
  return { step: 'model', currentLevel };
}

// Capabilities are injected from pi-ai so this flow follows the host's exact rules.
export function transition(state, event, capabilities) {
  // Escape and the ✕ button close the popup from either step.
  if (event.type === 'cancel') return { step: 'done', result: undefined };
  if (state.step === 'model' && event.type === 'model') {
    return { ...state, step: 'effort', model: event.model,
      levels: capabilities.getSupportedThinkingLevels(event.model),
      level: capabilities.clampThinkingLevel(event.model, state.currentLevel) };
  }
  if (state.step === 'effort' && event.type === 'level') {
    return { step: 'done', result: { model: state.model, level: event.level } };
  }
  return state;
}

const CLOSE = ' ✕ ';

export function createPicker(options, dependencies) {
  const { ModelSelectorComponent, ThinkingSelectorComponent, truncateToWidth, visibleWidth } = dependencies;
  const { tui, theme, model, runtime, scopedModels, defaultRef, done } = options;
  let state = createState(options.currentLevel);
  let child;
  let focused = false;
  let disposed = false;
  let closeAt = -1;

  const dispatch = event => {
    if (disposed) return;
    child?.dispose?.();
    state = transition(state, event, dependencies);
    if (state.step === 'done') {
      disposed = true;
      done(state.result);
      return;
    }
    mount();
    tui.requestRender();
  };
  const mount = () => {
    if (state.step === 'model') {
      child = new ModelSelectorComponent(tui, model, runtime, scopedModels,
        selected => dispatch({ type: 'model', model: selected }),
        () => dispatch({ type: 'cancel' }), undefined, undefined, defaultRef);
    } else {
      child = new ThinkingSelectorComponent(state.level, state.levels,
        level => dispatch({ type: 'level', level }), () => dispatch({ type: 'cancel' }));
    }
    child.focused = focused;
  };
  mount();
  return {
    get focused() { return focused; },
    set focused(value) { focused = value; child.focused = value; },
    get capturesViewportInput() { return child.capturesViewportInput; },
    handleInput(data) {
      if (disposed) return;
      child.handleInput(data);
      tui.requestRender();
    },
    // Mouse: only the ✕ in the top border is active; everything else is swallowed so clicks never leak below.
    handleMouse(event) {
      if (disposed || event.button !== 'left') return { handled: true };
      const onClose = event.y === 0 && event.x >= closeAt && event.x < closeAt + CLOSE.length;
      if (onClose && event.type === 'click') { dispatch({ type: 'cancel' }); return { handled: true, render: true }; }
      return { handled: true };
    },
    render(width) {
      const border = text => theme.fg('borderAccent', text);
      const inner = Math.max(1, width - 4);
      const fit = line => {
        const clipped = truncateToWidth(line, inner);
        return `${border('│')} ${clipped}${' '.repeat(Math.max(0, inner - visibleWidth(clipped)))} ${border('│')}`;
      };
      const title = state.step === 'effort' ? `Esfuerzo · ${state.model.id}` : 'Modelo';
      const head = truncateToWidth(` ${title} `, Math.max(0, width - 4 - CLOSE.length - 1), '…');
      const fill = Math.max(0, width - 4 - visibleWidth(head) - CLOSE.length);
      closeAt = 2 + visibleWidth(head) + fill;
      const top = `${border('╭─')}${theme.fg('accent', head)}${border('─'.repeat(fill))}${theme.fg('error', CLOSE)}${border('─╮')}`;
      const hint = truncateToWidth(' Esc cerrar ', Math.max(0, width - 4), '');
      const bottom = border(`╰─${hint}${'─'.repeat(Math.max(0, width - 4 - visibleWidth(hint)))}─╯`);
      // Native selectors draw their own full-width rules; the frame replaces them.
      const body = child.render(inner).filter(line => !/^─+$/.test(line.replace(/\x1b\[[0-9;:]*m/g, '')));
      return [top, ...body.map(fit), bottom];
    },
    invalidate() { child.invalidate(); },
    dispose() {
      if (disposed) return;
      disposed = true;
      child.dispose?.();
    },
  };
}

export function install(pi, dependencies) {
  let opened = false;
  const open = async ctx => {
    if (ctx.mode !== 'tui' || !ctx.hasUI) {
      ctx.ui.notify('El selector de modelo requiere el modo interactivo TUI.', 'warning');
      return;
    }
    if (opened) return;
    // ModelRegistry is a compatibility facade; Pi 1.5.0 exposes its ModelRuntime here.
    const runtime = ctx.modelRegistry.runtime;
    if (!runtime) {
      ctx.ui.notify('El selector no puede acceder al catálogo de modelos de esta versión de Pi.', 'error');
      return;
    }
    opened = true;
    try {
      const settings = pi.getSettings();
      const defaultRef = settings.defaultProvider && settings.defaultModel
        ? { provider: settings.defaultProvider, id: settings.defaultModel } : undefined;
      const result = await ctx.ui.custom((tui, theme, _keybindings, done) => createPicker({
        tui, theme, done, model: ctx.model, runtime, scopedModels: ctx.scopedModels,
        currentLevel: pi.getThinkingLevel(), defaultRef,
      }, dependencies), {
        overlay: true,
        overlayOptions: { anchor: 'center', width: '45%', minWidth: 44, maxHeight: '85%' },
      });
      if (!result) return;
      if (!await pi.setModel(result.model)) {
        ctx.ui.notify(`No se pudo activar el modelo: ${result.model.id}`, 'error');
        return;
      }
      pi.setThinkingLevel(result.level);
      ctx.ui.notify(`Modelo: ${result.model.id} · esfuerzo: ${result.level}`, 'info');
    } catch (error) {
      ctx.ui.notify(`Error del selector de modelo: ${error instanceof Error ? error.message : String(error)}`, 'error');
    } finally {
      opened = false;
    }
  };
  pi.registerShortcut('f9', { description: 'Choose model and effort', handler: open });
  pi.registerCommand('model-picker', {
    description: 'Choose model and effort in one popup (F9); Escape or ✕ closes',
    handler: async (_args, ctx) => open(ctx),
  });
}
