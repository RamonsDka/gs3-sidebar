import { ModelSelectorComponent, ThinkingSelectorComponent, type ExtensionAPI } from '@earendil-works/pi-coding-agent';
import { getSupportedThinkingLevels, clampThinkingLevel } from '@earendil-works/pi-ai';
import { truncateToWidth, visibleWidth } from '@earendil-works/pi-tui';
import { install } from './core.mjs';

export default function modelPicker(pi: ExtensionAPI) {
  install(pi, { ModelSelectorComponent, ThinkingSelectorComponent,
    getSupportedThinkingLevels, clampThinkingLevel, truncateToWidth, visibleWidth });
}
