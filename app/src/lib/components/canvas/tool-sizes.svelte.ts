import type { Tool } from '$lib/canvas/types';

export class ToolSizes {
	pen = $state(3);
	eraser = $state(20);
	text = $state(16);

	current(tool: Tool) {
		if (tool === 'eraser') return this.eraser;
		if (tool === 'text') return this.text;
		return this.pen;
	}

	set(tool: Tool, size: number) {
		if (tool === 'eraser') this.eraser = size;
		else if (tool === 'text') this.text = size;
		else this.pen = size;
	}
}
