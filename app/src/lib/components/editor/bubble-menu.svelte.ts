import type { Editor } from '@tiptap/core';
import { positionBubbleMenu } from '$lib/editor/handlers/bubble-menu';

export class BubbleMenu {
	visible = $state(false);
	element: HTMLElement | undefined = $state();
	private token = 0;

	hide(): void {
		this.visible = false;
		if (!this.element) return;
		this.element.style.left = '-9999px';
		this.element.style.top = '-9999px';
	}

	async update(editor: Editor | null): Promise<void> {
		if (!editor || !this.element) return;
		const token = ++this.token;
		const result = await positionBubbleMenu(editor, this.element);
		if (token !== this.token) return;
		if (result) {
			this.element.style.left = `${result.x}px`;
			this.element.style.top = `${result.y}px`;
			this.visible = true;
		} else {
			this.hide();
		}
	}
}
