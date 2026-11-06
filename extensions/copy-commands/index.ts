import { copyToClipboard, type ExtensionAPI } from '@earendil-works/pi-coding-agent';
import { MouseRegion, truncateToWidth } from '@earendil-works/pi-tui';
import { install } from './core.mjs';

export default function copyCommands(pi: ExtensionAPI) {
  install(pi, { copyToClipboard, MouseRegion, truncateToWidth });
}
