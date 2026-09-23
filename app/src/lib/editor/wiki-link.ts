import { Editor, Node, mergeAttributes, InputRule } from '@tiptap/core';
import { Plugin, PluginKey } from '@tiptap/pm/state';
import type { Node as PMNode, NodeType } from '@tiptap/pm/model';
import { extractWikiLinks, type TextNode } from '$lib/fs/bridge';

interface MarkdownSerializerState {
	write(content: string): void;
}

interface MdToken {
	content: string;
	attrPush(attr: [string, string]): void;
	attrGet(name: string): string | null;
}

interface MdInlineState {
	src: string;
	pos: number;
	posMax: number;
	push(type: string, tag: string, nesting: number): MdToken;
}

type MdInlineRule = (state: MdInlineState, silent: boolean) => boolean;

type MdRenderRule = (tokens: MdToken[], idx: number) => string;

interface MarkdownIt {
	utils: { escapeHtml(str: string): string };
	inline: { ruler: { push(name: string, rule: MdInlineRule): void } };
	renderer: { rules: Record<string, MdRenderRule> };
}

declare module '@tiptap/core' {
	interface Commands<ReturnType> {
		wikiLink: {
			insertWikiLink: (title: string) => ReturnType;
		};
	}
}

const WikiLink = Node.create({
	name: 'wikiLink',
	group: 'inline',
	inline: true,
	atom: true,
	selectable: true,

	addAttributes() {
		return {
			title: {
				default: null,
				parseHTML: (el) => (el as HTMLElement).getAttribute('data-title'),
				renderHTML: (attrs) => ({ 'data-title': attrs.title })
			}
		};
	},

	parseHTML() {
		return [{ tag: 'span[data-wiki-link]' }];
	},

	renderHTML({ node, HTMLAttributes }) {
		return [
			'span',
			mergeAttributes({ 'data-wiki-link': '', class: 'wiki-link' }, HTMLAttributes),
			['span', { class: 'wiki-link-icon', contenteditable: 'false' }],
			['span', { class: 'wiki-link-title' }, node.attrs.title]
		];
	},

	addCommands() {
		return {
			insertWikiLink:
				(title: string) =>
				({ chain }) => {
					return chain().focus().insertContent({ type: this.name, attrs: { title } }).run();
				}
		};
	},

	addInputRules() {
		return [
			new InputRule({
				find: /(?<!!)\[\[([^\][\n]+)\]\]$/,
				handler: ({ range, match, chain }) => {
					const title = match[1];
					if (!title) return null;
					chain().deleteRange(range).insertContent({ type: this.name, attrs: { title } }).run();
				}
			})
		];
	},

	onCreate() {
		const nodeType = this.type;
		const editor = this.editor;
		setTimeout(() => {
			convertWikiLinksAsync(editor, nodeType);
		}, 0);
	},

	addProseMirrorPlugins() {
		const nodeType = this.type;
		const editorRef = this.editor;
		const pluginKey = new PluginKey('wikiLinkTransform');
		return [
			new Plugin({
				key: pluginKey,
				appendTransaction(transactions, _oldState, _newState) {
					const isSetContent = transactions.some(
						(tr) => tr.docChanged && tr.getMeta('addToHistory') === false
					);
					if (!isSetContent) return null;
					convertWikiLinksAsync(editorRef, nodeType);
					return null;
				}
			})
		];
	},

	addStorage() {
		return {
			markdown: {
				serialize(state: MarkdownSerializerState, node: PMNode) {
					state.write(`[[${node.attrs.title}]]`);
				},
				parse: {
					setup(md: MarkdownIt) {
						wikiLinkPlugin(md);
					}
				}
			}
		};
	}
});

// onCreate and appendTransaction can fire together; a stale resolve would apply from/to on a changed doc.
let convertVersion = 0;

async function convertWikiLinksAsync(editor: Editor, nodeType: NodeType) {
	const version = ++convertVersion;
	const capturedDoc = editor.state.doc;

	const nodes: TextNode[] = [];
	capturedDoc.descendants((node, pos) => {
		if (!node.isText || !node.text) return;
		if (node.text.indexOf('[[') === -1) return;
		nodes.push({ text: node.text, pos });
	});
	if (nodes.length === 0) return;

	try {
		const replacements = await extractWikiLinks(nodes);
		if (replacements.length === 0) return;
		if (!editor.view || editor.isDestroyed) return;
		if (version !== convertVersion) return;
		if (editor.state.doc !== capturedDoc) return;
		const tr = editor.state.tr;
		for (let i = replacements.length - 1; i >= 0; i--) {
			const { from, to, title } = replacements[i];
			tr.replaceWith(from, to, nodeType.create({ title }));
		}
		tr.setMeta('addToHistory', false);
		editor.view.dispatch(tr);
	} catch {
		return;
	}
}

function wikiLinkPlugin(md: MarkdownIt) {
	md.inline.ruler.push('wiki_link', function (state, silent) {
		const src = state.src;
		const pos = state.pos;
		const max = state.posMax;

		if (pos + 3 > max) return false;
		if (pos > 0 && src.charCodeAt(pos - 1) === 0x21) return false;
		if (src.charCodeAt(pos) !== 0x5b) return false;
		if (src.charCodeAt(pos + 1) !== 0x5b) return false;

		const closePos = src.indexOf(']]', pos + 2);
		if (closePos === -1 || closePos >= max) return false;

		const title = src.slice(pos + 2, closePos).trim();
		if (!title) return false;

		if (silent) return true;

		const token = state.push('wiki_link', 'span', 0);
		token.attrPush(['data-wiki-link', '']);
		token.attrPush(['data-title', title]);
		token.attrPush(['class', 'wiki-link']);
		token.content = title;
		state.pos = closePos + 2;
		return true;
	});

	md.renderer.rules['wiki_link'] = function (tokens, idx) {
		const token = tokens[idx];
		const title = token.attrGet('data-title') ?? '';
		const escaped = md.utils.escapeHtml(title);
		return `<span data-wiki-link="" data-title="${escaped}" class="wiki-link"><span class="wiki-link-icon" contenteditable="false"></span><span class="wiki-link-title">${escaped}</span></span>`;
	};
}

export default WikiLink;
