import type { Editor } from '@tiptap/core';
import type { SourceEditor } from '$lib/editor/source/codemirror';
import { editor as editorStore } from '$lib/stores/editor.svelte';
import {
	reportSourceCursor,
	richCursorToSourceOffset,
	sourceOffsetToRichPos
} from './cursor-position';

export interface SourceSurfaceHost {
	serializeRich(editor: Editor): string;
	toEditorContent(markdown: string): string;
	flushSave(): void;
	scheduleSave(text: string): void;
	isAlive(): boolean;
	isActiveTab(): boolean;
	onEnter(): void;
}

export class SourceSurface {
	private readonly host: SourceSurfaceHost;
	seed: string | null;
	editor: SourceEditor | null = null;
	pendingCursor: number | null = null;
	private token = 0;

	constructor(host: SourceSurfaceHost, seed: string | null) {
		this.host = host;
		this.seed = seed;
	}

	async enter(rich: Editor, parent: HTMLElement | undefined): Promise<void> {
		const token = ++this.token;
		this.host.flushSave();
		this.host.onEnter();

		const text = this.seed ?? this.host.serializeRich(rich);
		let offset = richCursorToSourceOffset(rich, text);
		if (this.pendingCursor != null) {
			offset = this.pendingCursor;
			this.pendingCursor = null;
		}

		if (!this.editor) {
			if (!parent) return;
			const { createSourceEditor } = await import('$lib/editor/source/codemirror');
			if (token !== this.token || !this.host.isAlive()) return;
			this.editor = await createSourceEditor({
				parent,
				doc: text,
				onChange: (next) => this.host.scheduleSave(next),
				onCursor: (line, col) => editorStore.setCursor(line, col)
			});
			this.editor.setCursorOffset(offset);
		} else if (this.editor.getText() !== text) {
			this.editor.setText(text);
			this.editor.setCursorOffset(offset);
		} else {
			offset = this.editor.getCursorOffset();
		}

		if (token !== this.token || !this.host.isAlive()) return;

		reportSourceCursor(text, offset);
		if (this.host.isActiveTab()) this.editor.focus();
	}

	exit(rich: Editor): void {
		this.token++;
		this.host.flushSave();
		if (!this.editor) return;
		const text = this.editor.getText();
		const offset = this.editor.getCursorOffset();
		this.seed = text;
		if (!this.host.isAlive()) return;

		rich.commands.setContent(this.host.toEditorContent(text), { emitUpdate: false });
		const size = rich.state.doc.content.size;
		const caret = sourceOffsetToRichPos(rich, text, offset);
		rich.commands.setTextSelection(Math.min(Math.max(caret, 0), Math.max(size - 1, 0)));
		rich.commands.scrollIntoView();
	}

	destroy(): void {
		this.token++;
		this.editor?.destroy();
		this.editor = null;
	}
}
