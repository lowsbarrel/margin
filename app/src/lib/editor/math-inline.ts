import { Node, nodeInputRule, mergeAttributes } from '@tiptap/core';
import type { Node as PMNode } from '@tiptap/pm/model';
import { cachedKatex, escapeHtml, renderWhenReady } from '$lib/editor/math-render';
import {
	mathInlineMarkdownPlugin,
	type MarkdownIt,
	type MarkdownSerializerState
} from '$lib/editor/math-markdown';

declare module '@tiptap/core' {
	interface Commands<ReturnType> {
		mathInline: {
			setMathInline: (attrs?: { text?: string }) => ReturnType;
		};
	}
}

export const mathInlineInputRegex = /(?:^|\s)\$((?:[^$\s]|[^$\s][^$]*[^$\s]))\$$/;

export const MathInline = Node.create({
	name: 'mathInline',
	group: 'inline',
	inline: true,
	atom: true,
	selectable: true,

	addAttributes() {
		return {
			text: {
				default: '',
				parseHTML: (el: HTMLElement) => el.getAttribute('data-math-text') || el.textContent || ''
			}
		};
	},

	parseHTML() {
		return [
			{
				tag: 'span[data-type="mathInline"]'
			}
		];
	},

	renderHTML({ HTMLAttributes }) {
		const text = HTMLAttributes.text || '';
		let rendered: string;
		try {
			rendered = text
				? (cachedKatex(text, false) ?? escapeHtml(text))
				: '<span class="math-placeholder">math</span>';
		} catch {
			rendered = `<code class="math-error">${escapeHtml(text)}</code>`;
		}

		return [
			'span',
			mergeAttributes({
				'data-type': 'mathInline',
				'data-math-text': text,
				class: 'math-inline',
				contenteditable: 'false'
			}),
			rendered
		];
	},

	addNodeView() {
		return ({ node, getPos, editor }) => {
			const dom = document.createElement('span');
			dom.classList.add('math-inline');
			dom.setAttribute('data-type', 'mathInline');
			dom.contentEditable = 'false';

			let currentText = node.attrs.text || '';
			// The input takes over the whole node DOM, so a late KaTeX load must not repaint over it.
			let editing = false;

			function renderMath(text: string) {
				if (!text) {
					dom.innerHTML = '<span class="math-placeholder">math</span>';
					dom.classList.add('math-empty');
					return;
				}
				try {
					const html = cachedKatex(text, false);
					if (html === null) {
						dom.textContent = text;
						renderWhenReady(() => {
							if (!editing) renderMath(text);
						});
					} else {
						dom.innerHTML = html;
					}
				} catch {
					dom.textContent = text;
				}
				dom.classList.remove('math-empty');
			}

			renderMath(currentText);

			let openRafId: number | null = null;
			if (!currentText) {
				openRafId = requestAnimationFrame(() => {
					openRafId = null;
					if (!editor.isEditable) return;
					if (typeof getPos !== 'function' || getPos() == null) return;
					openEditor();
				});
			}

			function openEditor() {
				editing = true;
				const input = document.createElement('input');
				input.type = 'text';
				input.className = 'math-inline-input';
				input.value = currentText;
				input.placeholder = 'x^2 + y^2 = z^2';

				dom.innerHTML = '';
				dom.appendChild(input);
				input.focus();

				function commit() {
					editing = false;
					const newText = input.value.trim();
					currentText = newText;

					if (typeof getPos === 'function') {
						const pos = getPos();
						if (pos != null) {
							if (!newText) {
								editor.view.dispatch(editor.view.state.tr.delete(pos, pos + node.nodeSize));
								editor.view.focus();
								return;
							}
							editor.view.dispatch(
								editor.view.state.tr.setNodeMarkup(pos, undefined, {
									text: newText
								})
							);
						}
					}
					renderMath(currentText);
					editor.view.focus();
				}

				let committed = false;
				input.addEventListener('blur', () => {
					if (!committed) {
						committed = true;
						commit();
					}
				});
				input.addEventListener('keydown', (e) => {
					if (e.key === 'Enter' || e.key === 'Escape') {
						e.preventDefault();
						e.stopPropagation();
						if (e.key === 'Escape') {
							input.value = currentText;
						}
						committed = true;
						commit();
					}
				});
			}

			dom.addEventListener('click', (e) => {
				if (!editor.isEditable) return;
				e.stopPropagation();
				openEditor();
			});

			return {
				dom,
				update(updatedNode: PMNode) {
					if (updatedNode.type.name !== 'mathInline') return false;
					currentText = updatedNode.attrs.text || '';
					renderMath(currentText);
					return true;
				},
				stopEvent() {
					return true;
				},
				ignoreMutation() {
					return true;
				},
				destroy() {
					if (openRafId != null) {
						cancelAnimationFrame(openRafId);
						openRafId = null;
					}
				}
			};
		};
	},

	addCommands() {
		return {
			setMathInline:
				(attrs) =>
				({ commands }) =>
					commands.insertContent({
						type: 'mathInline',
						attrs: { text: attrs?.text ?? '' }
					})
		};
	},

	addInputRules() {
		return [
			nodeInputRule({
				find: mathInlineInputRegex,
				type: this.type,
				getAttributes: (match) => ({
					text: match[1]?.trim() || ''
				})
			})
		];
	},

	addStorage() {
		return {
			markdown: {
				serialize(state: MarkdownSerializerState, node: PMNode) {
					state.write(`$${node.attrs.text}$`);
				},
				parse: {
					setup(md: MarkdownIt) {
						mathInlineMarkdownPlugin(md);
					}
				}
			}
		};
	}
});
