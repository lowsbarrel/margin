import { Extension, type Editor, type Range } from '@tiptap/core';
import { PluginKey, type EditorState } from '@tiptap/pm/state';
import Suggestion, { type SuggestionOptions } from '@tiptap/suggestion';

export function notInCodeBlock(state: EditorState, range: Range): boolean {
	const $from = state.doc.resolve(range.from);
	return $from.parent.type.name !== 'codeBlock';
}

export interface SuggestionCommandProps<TItem> {
	editor: Editor;
	range: Range;
	props: TItem;
}

export interface CreateSuggestionExtensionOptions<TItem> {
	name: string;
	char: string;
	pluginKey: PluginKey;
	command: (props: SuggestionCommandProps<TItem>) => void;
}

export function createSuggestionExtension<TItem>(
	options: CreateSuggestionExtensionOptions<TItem>
): Extension {
	const { name, char, pluginKey, command } = options;

	return Extension.create({
		name,

		addOptions() {
			return {
				suggestion: {
					char,
					command,
					allow: ({ state, range }: { state: EditorState; range: Range }) =>
						notInCodeBlock(state, range)
				} as Partial<SuggestionOptions<TItem, TItem>>
			};
		},

		addProseMirrorPlugins() {
			return [
				Suggestion({
					pluginKey,
					...this.options.suggestion,
					editor: this.editor
				})
			];
		}
	});
}
