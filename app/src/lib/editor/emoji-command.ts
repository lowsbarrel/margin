import { PluginKey } from '@tiptap/pm/state';
import { EMOJIS, type EmojiItem } from '$lib/editor/emoji-data';
import { createSuggestionExtension } from '$lib/editor/suggestion-extension';

export type { EmojiItem };

export function getEmojiItems({ query }: { query: string }): EmojiItem[] {
	if (!query) return EMOJIS.slice(0, 20);
	const q = query.toLowerCase();
	return EMOJIS.filter((e) => e.name.includes(q) || e.keywords.some((k) => k.includes(q))).slice(
		0,
		20
	);
}

export const emojiCommandPluginKey = new PluginKey('emoji-command');

const EmojiCommand = createSuggestionExtension<EmojiItem>({
	name: 'emojiCommand',
	char: ':',
	pluginKey: emojiCommandPluginKey,
	command: ({ editor, range, props }) => {
		editor.chain().focus().deleteRange(range).insertContent(props.emoji).run();
	}
});

export default EmojiCommand;
