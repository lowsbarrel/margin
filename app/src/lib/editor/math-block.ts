import { Node, nodeInputRule, mergeAttributes } from '@tiptap/core';
import type { Node as PMNode } from '@tiptap/pm/model';
import * as m from '$lib/paraglide/messages.js';
import { cachedKatex, escapeHtml, renderWhenReady } from '$lib/editor/math-render';
import {
	mathBlockMarkdownPlugin,
	type MarkdownIt,
	type MarkdownSerializerState
} from '$lib/editor/math-markdown';

declare module '@tiptap/core' {
	interface Commands<ReturnType> {
		mathBlock: {
			setMathBlock: (attrs?: { text?: string }) => ReturnType;
		};
	}
}

export const mathBlockInputRegex = /(?:^|\s)((?:\$\$\$)((?:[^$]+))(?:\$\$\$))$/;

function autoResize(textarea: HTMLTextAreaElement) {
	textarea.style.height = 'auto';
	textarea.style.height = textarea.scrollHeight + 'px';
}

export const MathBlock = Node.create({
	name: 'mathBlock',
	group: 'block',
	atom: true,
	isolating: true,
	selectable: true,
	draggable: true,

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
				tag: 'div[data-type="mathBlock"]'
			}
		];
	},

	renderHTML({ HTMLAttributes }) {
		const text = HTMLAttributes.text || '';
		let rendered: string;
		try {
			rendered = cachedKatex(text, true) ?? escapeHtml(text);
		} catch {
			rendered = `<code class="math-error">${escapeHtml(text)}</code>`;
		}

		return [
			'div',
			mergeAttributes({
				'data-type': 'mathBlock',
				'data-math-text': text,
				class: 'math-block',
				contenteditable: 'false'
			}),
			['div', { class: 'math-render', innerHTML: rendered }]
		];
	},

	addNodeView() {
		return ({ node, getPos, editor }) => {
			const dom = document.createElement('div');
			dom.classList.add('math-block');
			dom.setAttribute('data-type', 'mathBlock');
			dom.contentEditable = 'false';

			const renderArea = document.createElement('div');
			renderArea.classList.add('math-render');

			const inputArea = document.createElement('textarea');
			inputArea.classList.add('math-input');
			inputArea.value = node.attrs.text || '';
			inputArea.placeholder = 'E = mc^2';
			inputArea.rows = 1;
			inputArea.style.display = 'none';

			function renderMath(text: string) {
				try {
					const html = cachedKatex(text, true);
					if (html === null) {
						renderArea.textContent = text;
						renderWhenReady(() => renderMath(text));
					} else {
						renderArea.innerHTML = html;
					}
				} catch {
					renderArea.textContent = text || m.editor_empty_math();
				}
				renderArea.classList.toggle('math-empty', !text);
			}

			renderMath(node.attrs.text || '');
			dom.addEventListener('click', (e) => {
				if (!editor.isEditable) return;
				e.stopPropagation();
				inputArea.style.display = '';
				renderArea.style.display = 'none';
				inputArea.focus();
				autoResize(inputArea);
			});

			inputArea.addEventListener('input', () => {
				autoResize(inputArea);
			});

			inputArea.addEventListener('blur', () => {
				const newText = inputArea.value;
				inputArea.style.display = 'none';
				renderArea.style.display = '';
				renderMath(newText);
				if (typeof getPos === 'function') {
					const pos = getPos();
					if (pos != null) {
						editor.view.dispatch(
							editor.view.state.tr.setNodeMarkup(pos, undefined, {
								text: newText
							})
						);
					}
				}
			});

			inputArea.addEventListener('keydown', (e) => {
				if (e.key === 'Escape' || (e.key === 'Enter' && !e.shiftKey)) {
					e.preventDefault();
					inputArea.blur();
				}
			});

			dom.appendChild(renderArea);
			dom.appendChild(inputArea);

			return {
				dom,
				update(updatedNode: PMNode) {
					if (updatedNode.type.name !== 'mathBlock') return false;
					if (inputArea.style.display === 'none') {
						inputArea.value = updatedNode.attrs.text || '';
						renderMath(updatedNode.attrs.text || '');
					}
					return true;
				},
				stopEvent() {
					return true;
				},
				ignoreMutation() {
					return true;
				}
			};
		};
	},

	addCommands() {
		return {
			setMathBlock:
				(attrs) =>
				({ commands }) =>
					commands.insertContent({
						type: 'mathBlock',
						attrs: { text: attrs?.text ?? '' }
					})
		};
	},

	addInputRules() {
		return [
			nodeInputRule({
				find: mathBlockInputRegex,
				type: this.type,
				getAttributes: (match) => ({
					text: match[2]?.trim() || ''
				})
			})
		];
	},

	addStorage() {
		return {
			markdown: {
				serialize(state: MarkdownSerializerState, node: PMNode) {
					state.write(`$$\n${node.attrs.text}\n$$`);
					state.closeBlock(node);
				},
				parse: {
					setup(md: MarkdownIt) {
						mathBlockMarkdownPlugin(md);
					}
				}
			}
		};
	}
});
