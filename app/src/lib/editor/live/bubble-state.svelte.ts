import type { BlockType } from './commands';
import type { MarkKind } from './marks';

export class ToolbarState {
	marks = $state<MarkKind[]>([]);
	block = $state<BlockType>('text');
}
