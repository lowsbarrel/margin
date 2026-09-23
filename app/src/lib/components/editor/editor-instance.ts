import { Editor } from '@tiptap/core';
import { createEditorExtensions, type Lowlight } from '$lib/editor/extensions';

export interface EditorInstanceOptions {
	element: HTMLElement;
	content: string;
	lowlight: Lowlight | null;
	attachmentFolder: string;
	serialize(editor: Editor): string;
	onDocumentChange(markdown: string): void;
	onSelectionChange(): void;
	onBlur(): void;
	onFocus(editor: Editor): void;
}

export function createEditorInstance(options: EditorInstanceOptions): Editor {
	let lastSavedMarkdown: string | null = null;

	return new Editor({
		element: options.element,
		extensions: createEditorExtensions({
			lowlight: options.lowlight,
			attachmentFolder: options.attachmentFolder
		}),
		content: options.content,
		editorProps: {
			attributes: {
				class: 'md-editor',
				spellcheck: 'false'
			}
		},
		onCreate: ({ editor }) => {
			lastSavedMarkdown = options.serialize(editor);
		},
		onUpdate: ({ editor }) => {
			const markdown = options.serialize(editor);
			if (markdown === lastSavedMarkdown) return;
			lastSavedMarkdown = markdown;
			options.onDocumentChange(markdown);
		},
		onSelectionUpdate: options.onSelectionChange,
		onBlur: options.onBlur,
		onFocus: ({ editor }) => options.onFocus(editor)
	});
}
