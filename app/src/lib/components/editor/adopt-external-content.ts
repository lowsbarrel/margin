import type { Editor } from '@tiptap/core';
import type { ViewMode } from '$lib/stores/panes.svelte';
import { editor as editorStore } from '$lib/stores/editor.svelte';
import type { NoteDocument } from './note-document';
import type { SaveController } from './save-controller';
import type { SourceSurface } from './source-surface';

export interface AdoptExternalContentOptions {
	content: string;
	mode: ViewMode;
	rich: Editor;
	note: NoteDocument;
	save: SaveController;
	source: SourceSurface;
}

export function adoptExternalContent({
	content,
	mode,
	rich,
	note,
	save,
	source
}: AdoptExternalContentOptions): void {
	const sourceEditor = mode === 'source' ? source.editor : null;
	save.snapshot(sourceEditor ? sourceEditor.getText() : note.serialize(rich));
	save.cancel();
	editorStore.setDirty(false);
	source.seed = content;
	save.lastSavedText = content;

	if (sourceEditor) {
		sourceEditor.setText(content);
		return;
	}

	const previous = rich.state.selection;
	rich.commands.setContent(note.toEditor(content), { emitUpdate: false });
	const size = rich.state.doc.content.size;
	rich.commands.setTextSelection({
		from: Math.min(previous.from, size),
		to: Math.min(previous.to, size)
	});
}
