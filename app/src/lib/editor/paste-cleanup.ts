import { Extension } from '@tiptap/core';
import { Plugin, PluginKey } from '@tiptap/pm/state';
import { Fragment, Slice } from '@tiptap/pm/model';
import type { Node as ProseMirrorNode } from '@tiptap/pm/model';
import { encodeLocalfileImageSpaces } from '$lib/editor/image-url';

const YAML_FRONT_MATTER_REGEX = /^\s*---[\s\S]*?---\s*/;

const APPEARANCE_MARKS = new Set([
	'textStyle',
	'highlight',
	'underline',
	'superscript',
	'subscript'
]);

const APPEARANCE_ATTRS = ['textAlign'];

function stripNodeAppearance(node: ProseMirrorNode): ProseMirrorNode {
	let next = node;

	if (next.marks.length) {
		const kept = next.marks.filter((mark) => !APPEARANCE_MARKS.has(mark.type.name));
		if (kept.length !== next.marks.length) next = next.mark(kept);
	}

	const content = next.content.size ? sanitizeFragment(next.content) : next.content;

	let attrs: Record<string, unknown> | null = null;
	for (const attr of APPEARANCE_ATTRS) {
		if (next.attrs && attr in next.attrs) {
			const def = next.type.spec.attrs?.[attr]?.default ?? null;
			if (next.attrs[attr] !== def) {
				attrs ??= { ...next.attrs };
				attrs[attr] = def;
			}
		}
	}

	if (attrs) return next.type.create(attrs, content, next.marks);
	if (content !== next.content) return next.copy(content);
	return next;
}

function sanitizeFragment(fragment: Fragment): Fragment {
	const children: ProseMirrorNode[] = [];
	let changed = false;
	fragment.forEach((node) => {
		const clean = stripNodeAppearance(node);
		if (clean !== node) changed = true;
		children.push(clean);
	});
	return changed ? Fragment.fromArray(children) : fragment;
}

function sanitizeSlice(slice: Slice): Slice {
	const content = sanitizeFragment(slice.content);
	if (content === slice.content) return slice;
	return new Slice(content, slice.openStart, slice.openEnd);
}

function stripTrailingEmptyParagraphs(slice: Slice): Slice {
	let { content } = slice;
	while (content.childCount > 1) {
		const lastChild = content.lastChild;
		if (lastChild?.type.name === 'paragraph' && lastChild.textContent.trim() === '') {
			const children: ProseMirrorNode[] = [];
			for (let i = 0; i < content.childCount - 1; i++) children.push(content.child(i));
			content = Fragment.fromArray(children);
		} else {
			break;
		}
	}

	if (content === slice.content) return slice;
	return new Slice(content, slice.openStart, Math.max(slice.openEnd, 1));
}

export const PasteCleanup = Extension.create({
	name: 'pasteCleanup',
	// Above tiptap-markdown's 50, so handlePaste claims the event first.
	priority: 60,

	addProseMirrorPlugins() {
		return [
			new Plugin({
				key: new PluginKey('pasteCleanup'),
				props: {
					handlePaste: (view, event) => {
						if (this.editor.isActive('codeBlock')) {
							return false;
						}

						const text = event.clipboardData?.getData('text/plain');
						const html = event.clipboardData?.getData('text/html');
						if (text && !html && YAML_FRONT_MATTER_REGEX.test(text)) {
							const cleaned = text.replace(YAML_FRONT_MATTER_REGEX, '').trimStart();
							if (cleaned !== text) {
								const { from, to } = view.state.selection;
								const tr = view.state.tr.insertText(cleaned, from, to);
								view.dispatch(tr);
								return true;
							}
						}

						return false;
					},

					transformPastedText: (text) => encodeLocalfileImageSpaces(text),

					transformPasted: (slice) => stripTrailingEmptyParagraphs(sanitizeSlice(slice)),

					transformCopied: (slice) => sanitizeSlice(slice)
				}
			})
		];
	}
});
