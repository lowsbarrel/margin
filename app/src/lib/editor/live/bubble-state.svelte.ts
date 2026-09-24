import type { BlockType } from './commands';

export class ToolbarState {
	marks = $state<string[]>([]);
	block = $state<BlockType>('text');
}
