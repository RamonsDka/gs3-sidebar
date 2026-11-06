import { UserMessageComponent, AssistantMessageComponent, ToolExecutionComponent, BashExecutionComponent, CustomMessageComponent, CompactionSummaryMessageComponent, BranchSummaryMessageComponent, SkillInvocationMessageComponent, InteractiveMode, type ExtensionAPI } from '@earendil-works/pi-coding-agent';
import * as tui from '@earendil-works/pi-tui';
import { installFrames } from './frame.mjs';

export default function messageFrames(pi: ExtensionAPI) {
 let controller: ReturnType<typeof installFrames> | undefined;
 let disabled = false;
 const enable = (ctx: any) => {
  if (ctx.mode !== 'tui' || controller) return;
  try {
   // CustomEntryComponent is not exported. Use the host's own chat route to
   // capture its actual constructor without loading another package instance.
   let CustomEntryComponent: any;
   InteractiveMode.prototype.addCustomEntryToChat.call({
    session: {extensionRunner: {getEntryRenderer: () => () => new tui.Container()}},
    outputPad: 1, toolOutputExpanded: false,
    chatContainer: {addChild: (component: any) => { CustomEntryComponent = component.constructor; }},
   } as any, {customType: 'message-frames-probe'} as any);
   controller = installFrames([
    [UserMessageComponent, () => 'User', [91, 174, 230]],
    [AssistantMessageComponent, () => 'Assistant', [156, 136, 224]],
    [ToolExecutionComponent, (c: any) => c.toolName === 'bash' ? 'Bash (tool)' : `Tool: ${c.toolName}`, [215, 170, 91]],
    [BashExecutionComponent, () => 'Bash', [215, 170, 91]],
    [CustomMessageComponent, (c: any) => /agent/i.test(c.message?.customType ?? '') ? 'Subagent' : 'Custom', [104, 191, 156]],
    [CompactionSummaryMessageComponent, () => 'Compaction', [156, 136, 224]],
    [BranchSummaryMessageComponent, () => 'Branch', [156, 136, 224]],
    [SkillInvocationMessageComponent, () => 'Skill', [91, 174, 230]],
    [CustomEntryComponent, () => 'Custom entry', [104, 191, 156]],
   ], tui);
  } catch (error) {
   ctx.ui.notify(`Message frames disabled: ${String(error)}`, 'error');
  }
 };
 pi.on('session_start', (_event,ctx) => { if (!disabled) enable(ctx); });
 pi.on('session_shutdown', () => { controller?.restore(); controller = undefined; });
 pi.registerCommand('message-frames', {
  description: 'Message frames: on | off | status (presentation only; default on after reload)',
  handler: async (args,ctx) => {
   const action = args.trim() || 'status';
   if (!['on','off','status'].includes(action)) {
    ctx.ui.notify('Usage: /message-frames on|off|status', 'error'); return;
   }
   if (action === 'off') { disabled = true; controller?.restore(); controller = undefined; }
   if (action === 'on') { disabled = false; enable(ctx); }
   ctx.ui.notify(`Message frames: ${controller ? 'on' : 'off'}`, 'info');
  },
 });
}
