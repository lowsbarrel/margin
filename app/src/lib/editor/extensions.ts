import StarterKit from '@tiptap/starter-kit';
import TableRow from '@tiptap/extension-table-row';
import TableCell from '@tiptap/extension-table-cell';
import TableHeader from '@tiptap/extension-table-header';
import TaskList from '@tiptap/extension-task-list';
import TaskItem from '@tiptap/extension-task-item';
import Placeholder from '@tiptap/extension-placeholder';
import Typography from '@tiptap/extension-typography';
import markdownItMark from 'markdown-it-mark';
import Link from '@tiptap/extension-link';
import CharacterCount from '@tiptap/extension-character-count';
import CodeBlockLowlight from '@tiptap/extension-code-block-lowlight';
import Image from '@tiptap/extension-image';
import { Markdown } from 'tiptap-markdown';
import FileEmbed from '$lib/editor/file-embed';
import NoteEmbed from '$lib/editor/note-embed';
import WikiLink from '$lib/editor/wiki-link';
import Callout from '$lib/editor/callout';
import { TableMarkdown } from '$lib/editor/table-markdown';
import { MathBlock } from '$lib/editor/math-block';
import { MathInline } from '$lib/editor/math-inline';
import { Mermaid } from '$lib/editor/mermaid';
import SlashCommand from '$lib/editor/slash-command';
import { getSlashMenuItems } from '$lib/editor/menu-items';
import renderSlashMenu from '$lib/editor/slash-menu-renderer.svelte';
import MentionCommand from '$lib/editor/mention-command';
import renderMentionMenu from '$lib/editor/mention-menu-renderer.svelte';
import { SearchReplace } from '$lib/editor/search-replace';
import { ContentDrag } from '$lib/editor/content-drag';
import { PasteCleanup } from '$lib/editor/paste-cleanup';
import { TrailingNode } from '$lib/editor/trailing-node';
import { SelectionPreserve } from '$lib/editor/selection-preserve';
import { TableDndExtension } from '$lib/editor/table-dnd';
import { Extension, type Extensions } from '@tiptap/core';
import type { Fragment } from '@tiptap/pm/model';
import { Plugin, PluginKey } from '@tiptap/pm/state';
import type { createLowlight } from 'lowlight';
import * as m from '$lib/paraglide/messages.js';

export type Lowlight = ReturnType<typeof createLowlight>;

interface MarkdownIt {
	use(plugin: unknown): MarkdownIt;
}

declare module 'tiptap-markdown' {
	interface MarkdownStorage {
		serializer?: { serialize(content: Fragment): string };
		parser?: { parse(markdown: string): string };
	}
}

interface CreateExtensionsOptions {
	lowlight: Lowlight | null;
	attachmentFolder?: string | null;
	embed?: boolean;
	embedDepth?: number;
}

const LegacyHighlightStrip = Extension.create({
	name: 'legacyHighlightStrip',
	addStorage() {
		return {
			markdown: {
				parse: {
					setup(md: MarkdownIt) {
						md.use(markdownItMark);
					}
				}
			}
		};
	}
});

export function createEditorExtensions({
	lowlight,
	attachmentFolder,
	embed = false,
	embedDepth = 0
}: CreateExtensionsOptions): Extensions {
	const exts: Extensions = [
		StarterKit.configure({
			heading: { levels: [1, 2, 3, 4, 5, 6] },
			codeBlock: false,
			link: false,
			underline: false,
			trailingNode: false,
			dropcursor: {
				width: 3,
				color: '#70CFF8'
			}
		}),
		CodeBlockLowlight.configure({
			lowlight,
			HTMLAttributes: {
				spellcheck: 'false'
			}
		}),
		TableMarkdown.configure({
			resizable: true,
			lastColumnResizable: true,
			allowTableNodeSelection: true
		}),
		TableRow,
		TableCell,
		TableHeader,
		TaskList,
		TaskItem.extend({
			addKeyboardShortcuts() {
				return {
					...this.parent?.(),
					Backspace: ({ editor }) => {
						const { state } = editor;
						const { selection } = state;
						if (!selection.empty) return false;

						const { $from } = selection;
						if ($from.parentOffset !== 0) return false;

						let taskItemDepth: number | null = null;
						for (let d = $from.depth; d > 0; d--) {
							if ($from.node(d).type.name === 'taskItem') {
								taskItemDepth = d;
								break;
							}
						}
						if (taskItemDepth === null) return false;

						const taskListDepth = taskItemDepth - 1;
						if (taskListDepth < 1) return false;
						const taskListNode = $from.node(taskListDepth);
						if (taskListNode.type.name !== 'taskList') return false;

						const parentDepth = taskListDepth - 1;
						if (parentDepth < 1) return false;
						const parentNode = $from.node(parentDepth);
						if (parentNode.type.name !== 'listItem') return false;

						const taskItemNode = $from.node(taskItemDepth);

						if (taskItemNode.content.size <= 2) {
							if (taskListNode.childCount === 1) {
								const taskListPos = $from.before(taskListDepth);
								const tr = state.tr.delete(taskListPos, taskListPos + taskListNode.nodeSize);
								editor.view.dispatch(tr);
							} else {
								const taskItemPos = $from.before(taskItemDepth);
								const tr = state.tr.delete(taskItemPos, taskItemPos + taskItemNode.nodeSize);
								editor.view.dispatch(tr);
							}
							return true;
						}

						return editor.commands.liftListItem('taskItem');
					}
				};
			}
		}).configure({
			nested: true
		}),
		...(embed
			? []
			: [
					Placeholder.configure({
						placeholder: ({ node }) => {
							if (node.type.name === 'heading') {
								return m.editor_placeholder_heading({ level: node.attrs.level });
							}
							return m.editor_placeholder_body();
						},
						includeChildren: true,
						showOnlyWhenEditable: true
					})
				]),
		Typography,
		Link.configure({
			openOnClick: false
		}),
		LegacyHighlightStrip,
		CharacterCount,
		...(embed
			? []
			: [
					SlashCommand.configure({
						suggestion: {
							items: getSlashMenuItems,
							render: renderSlashMenu
						}
					})
				]),
		Image.configure({
			inline: false,
			allowBase64: true
		}),
		// NoteEmbed must precede FileEmbed so it claims `![[Note]]` / `![[Note.md]]` first.
		NoteEmbed.configure({
			attachmentFolder: attachmentFolder || 'attachments',
			lowlight,
			depth: embedDepth
		}),
		FileEmbed.configure({
			attachmentFolder: attachmentFolder || 'attachments'
		}),
		...(embed
			? []
			: [
					MentionCommand.configure({
						suggestion: {
							items: () => [{ id: 'placeholder' }],
							render: renderMentionMenu
						}
					})
				]),
		WikiLink,
		Callout,
		MathBlock,
		MathInline,
		Mermaid,
		Markdown.configure({
			html: true,
			transformPastedText: true,
			transformCopiedText: false
		})
	];

	if (!embed) {
		exts.push(
			Extension.create({
				name: 'selectiveClipboardMarkdown',
				priority: 100,
				addProseMirrorPlugins() {
					return [
						new Plugin({
							key: new PluginKey('selectiveClipboardMarkdown'),
							props: {
								// ProseMirror reads '' as "no handler" and falls back to its own text extraction.
								clipboardTextSerializer: (slice) => {
									const listTypes = ['bulletList', 'orderedList', 'taskList'];
									let topLevelCount = 0;
									let hasList = false;
									slice.content.forEach((node) => {
										if (listTypes.includes(node.type.name)) {
											hasList = true;
											topLevelCount += node.childCount;
										} else {
											topLevelCount++;
										}
									});
									if (!hasList || topLevelCount < 2) return '';
									const serializer = this.editor.storage.markdown?.serializer;
									if (!serializer) return '';
									return serializer.serialize(slice.content);
								}
							}
						})
					];
				}
			}),
			PasteCleanup,
			SearchReplace,
			ContentDrag,
			TableDndExtension,
			TrailingNode,
			SelectionPreserve
		);
	}

	return exts;
}
