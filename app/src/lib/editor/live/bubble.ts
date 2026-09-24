import { computePosition, flip, offset, shift } from '@floating-ui/dom';
import { syntaxTree } from '@codemirror/language';
import type { EditorState } from '@codemirror/state';
import type { SyntaxNode } from '@lezer/common';
import { ViewPlugin, type EditorView, type ViewUpdate } from '@codemirror/view';
import { getLocale } from '$lib/paraglide/runtime.js';
import * as m from '$lib/paraglide/messages.js';
import { setLink, toggleHeading, toggleList, toggleMark, toggleQuote } from './commands';
import { ESCAPE_BUBBLE, onEscape } from './escape';
import { HIGHLIGHT } from './syntax';
import './assist.css';

interface BubbleButton {
	cmd: string;
	title: () => string;
	text: string;
	style: string;
}

const BUTTONS: (BubbleButton | 'separator')[] = [
	{ cmd: 'bold', title: () => m.bubble_bold(), text: 'B', style: 'cm-assist-b' },
	{ cmd: 'italic', title: () => m.bubble_italic(), text: 'I', style: 'cm-assist-i' },
	{ cmd: 'strike', title: () => m.bubble_strike(), text: 'S', style: 'cm-assist-s' },
	{ cmd: 'code', title: () => m.bubble_code(), text: '</>', style: 'cm-assist-mono' },
	{ cmd: 'highlight', title: () => m.bubble_highlight(), text: '==', style: 'cm-assist-mark' },
	'separator',
	{ cmd: 'quote', title: () => m.bubble_block_quote(), text: '"', style: 'cm-assist-mono' },
	{ cmd: 'bullet', title: () => m.bubble_block_bullet(), text: '•', style: 'cm-assist-mono' },
	{ cmd: 'task', title: () => m.bubble_block_task(), text: '☑', style: 'cm-assist-mono' },
	'separator',
	{ cmd: 'link', title: () => m.bubble_link(), text: '🔗', style: 'cm-assist-mono' }
];

const SIZE_LABELS = ['H1', 'H2', 'H3', 'H4', 'H5', 'H6'];
const MARKER_NODES: Record<string, string> = {
	StrongEmphasis: 'bold',
	Emphasis: 'italic',
	Strikethrough: 'strike',
	InlineCode: 'code',
	[HIGHLIGHT]: 'highlight',
	Blockquote: 'quote',
	BulletList: 'bullet'
};
const CODE_NODES: Record<string, true> = { FencedCode: true, CodeBlock: true };

function ancestorNames(state: EditorState): Set<string> {
	const { from, to } = state.selection.main;
	const names = new Set<string>();
	const tree = syntaxTree(state);
	for (const pos of to === from ? [from] : [from, to]) {
		let node: SyntaxNode | null = tree.resolveInner(pos, 1);
		while (node) {
			if (node.from <= from && node.to >= to) names.add(node.name);
			node = node.parent;
		}
	}
	return names;
}

function insideCode(state: EditorState): boolean {
	let node: SyntaxNode | null = syntaxTree(state).resolveInner(state.selection.main.head, -1);
	while (node) {
		if (CODE_NODES[node.name]) return true;
		node = node.parent;
	}
	return false;
}

function activeState(state: EditorState): { marks: Set<string>; heading: number } {
	const names = ancestorNames(state);
	const marks = new Set<string>();
	for (const [name, mark] of Object.entries(MARKER_NODES)) if (names.has(name)) marks.add(mark);
	if (names.has('Task')) {
		marks.add('task');
		marks.delete('bullet');
	}
	let heading = 0;
	for (let level = 1; level <= 6; level++) {
		if (names.has(`ATXHeading${level}`) || names.has(`SetextHeading${level}`)) heading = level;
	}
	return { marks, heading };
}

function command(view: EditorView, name: string): void {
	if (name === 'bold') toggleMark(view, '**');
	else if (name === 'italic') toggleMark(view, '*');
	else if (name === 'strike') toggleMark(view, '~~');
	else if (name === 'code') toggleMark(view, '`');
	else if (name === 'highlight') toggleMark(view, '==');
	else if (name === 'quote') toggleQuote(view);
	else if (name === 'bullet') toggleList(view, 'bullet');
	else if (name === 'task') toggleList(view, 'task');
}

function selectionRect(view: EditorView): DOMRect | null {
	const selection = window.getSelection();
	if (selection && selection.rangeCount > 0) {
		const range = selection.getRangeAt(0);
		if (!range.collapsed && view.dom.contains(range.startContainer)) {
			const rect = range.getBoundingClientRect();
			if (rect.width > 0 || rect.height > 0) return rect;
		}
	}
	const { from, to } = view.state.selection.main;
	const start = view.coordsAtPos(from);
	const end = view.coordsAtPos(to, -1);
	if (!start || !end) return null;
	return DOMRect.fromRect({
		x: Math.min(start.left, end.left),
		y: Math.min(start.top, end.top),
		width: Math.max(end.right, start.right) - Math.min(start.left, end.left),
		height: Math.max(end.bottom, start.bottom) - Math.min(start.top, end.top)
	});
}

export const liveBubble = ViewPlugin.fromClass(
	class {
		readonly bubble: HTMLElement;
		readonly buttons: HTMLButtonElement[] = [];
		heading!: HTMLSelectElement;
		linkRow!: HTMLElement;
		linkInput!: HTMLInputElement;
		frame = 0;
		dragging = false;
		suppressed = false;
		locale = getLocale();
		onRelease = () => {
			this.dragging = false;
			this.schedule();
		};
		releaseEscape: () => void;
		// The editor scrolls in an outer container, so CodeMirror's own scroll events never fire.
		onScroll = () => this.schedule();

		constructor(readonly view: EditorView) {
			this.bubble = document.createElement('div');
			this.bubble.className = 'cm-assist-bubble';
			this.bubble.setAttribute('role', 'toolbar');
			for (const entry of BUTTONS) {
				if (entry === 'separator') {
					const separator = document.createElement('span');
					separator.className = 'cm-assist-sep';
					this.bubble.appendChild(separator);
					continue;
				}
				const button = document.createElement('button');
				button.type = 'button';
				button.className = `cm-assist-btn ${entry.style}`;
				button.dataset.cmd = entry.cmd;
				button.textContent = entry.text;
				this.bubble.appendChild(button);
				this.buttons.push(button);
			}
			this.heading = document.createElement('select');
			this.heading.className = 'cm-assist-select';
			this.bubble.insertBefore(this.heading, this.buttons[5]);
			this.heading.addEventListener('change', () => {
				const level = Number(this.heading.value);
				const current = activeState(this.view.state).heading;
				if (level > 0) toggleHeading(this.view, level);
				else if (current > 0) toggleHeading(this.view, current);
				this.view.focus();
				this.schedule();
			});
			this.setupLinkRow();
			this.refreshLabels();
			this.bubble.addEventListener('mousedown', (event) => {
				const target = event.target as HTMLElement;
				if (target.closest('select, input')) return;
				event.preventDefault();
			});
			this.bubble.addEventListener('click', (event) => this.onClick(event));
			document.body.appendChild(this.bubble);
			window.addEventListener('mouseup', this.onRelease);
			window.addEventListener('scroll', this.onScroll, true);
			this.releaseEscape = onEscape(view, ESCAPE_BUBBLE, () => {
				if (!this.bubble.classList.contains('is-visible')) return false;
				this.suppressed = true;
				this.hide();
				return true;
			});
		}

		setupLinkRow(): void {
			this.linkRow = document.createElement('div');
			this.linkRow.className = 'cm-assist-link-row';
			this.linkInput = document.createElement('input');
			this.linkInput.className = 'cm-assist-input';
			this.linkInput.type = 'url';
			this.linkInput.placeholder = 'https://...';
			this.linkRow.appendChild(this.linkInput);
			const confirm = document.createElement('button');
			confirm.type = 'button';
			confirm.className = 'cm-assist-btn cm-assist-confirm';
			confirm.textContent = '✓';
			const cancel = document.createElement('button');
			cancel.type = 'button';
			cancel.className = 'cm-assist-btn';
			cancel.textContent = '✕';
			this.linkRow.append(confirm, cancel);
			this.bubble.appendChild(this.linkRow);
			confirm.addEventListener('click', () => this.submitLink());
			cancel.addEventListener('click', () => this.closeLinkRow());
			this.linkInput.addEventListener('keydown', (event) => {
				if (event.key === 'Enter') {
					event.preventDefault();
					this.submitLink();
				} else if (event.key === 'Escape') {
					event.preventDefault();
					this.closeLinkRow();
				}
			});
		}

		refreshLabels(): void {
			for (const button of this.buttons) {
				const entry = BUTTONS.find(
					(item) => item !== 'separator' && item.cmd === button.dataset.cmd
				) as BubbleButton | undefined;
				if (entry) {
					button.title = entry.title();
					button.setAttribute('aria-label', entry.title());
				}
			}
			this.heading.title = m.bubble_block_type();
			this.heading.replaceChildren();
			const text = document.createElement('option');
			text.value = '0';
			text.textContent = m.bubble_text();
			this.heading.appendChild(text);
			SIZE_LABELS.forEach((label, index) => {
				const option = document.createElement('option');
				option.value = String(index + 1);
				option.textContent = label;
				this.heading.appendChild(option);
			});
		}

		onClick(event: MouseEvent): void {
			const button = (event.target as HTMLElement).closest('button');
			if (!button) return;
			const name = button.dataset.cmd;
			if (name === 'link') {
				this.linkRow.classList.toggle('is-open');
				if (this.linkRow.classList.contains('is-open')) {
					this.linkInput.value = '';
					this.linkInput.focus();
				}
				return;
			}
			if (!name) return;
			command(this.view, name);
			this.view.focus();
			this.schedule();
		}

		submitLink(): void {
			const url = this.linkInput.value.trim();
			if (url) setLink(this.view, url);
			this.closeLinkRow();
			this.view.focus();
		}

		closeLinkRow(): void {
			this.linkRow.classList.remove('is-open');
			this.view.focus();
		}

		schedule(): void {
			if (this.frame) return;
			this.frame = requestAnimationFrame(() => {
				this.frame = 0;
				void this.place();
			});
		}

		hide(): void {
			this.bubble.classList.remove('is-visible');
		}

		async place(): Promise<void> {
			const { view } = this;
			const range = view.state.selection.main;
			if (range.empty || this.dragging || this.suppressed || insideCode(view.state)) {
				this.hide();
				return;
			}
			const rect = selectionRect(view);
			const scroller = view.scrollDOM.getBoundingClientRect();
			const top = Math.max(scroller.top, 0);
			const bottom = Math.min(scroller.bottom, window.innerHeight);
			const left = Math.max(scroller.left, 0);
			const right = Math.min(scroller.right, window.innerWidth);
			if (
				!rect ||
				rect.bottom < top ||
				rect.top > bottom ||
				rect.right < left ||
				rect.left > right
			) {
				this.hide();
				return;
			}
			const active = activeState(view.state);
			for (const button of this.buttons) {
				button.classList.toggle('is-active', active.marks.has(button.dataset.cmd ?? ''));
			}
			this.heading.value = String(active.heading);
			this.heading.classList.toggle('is-active', active.heading > 0);
			if (this.locale !== getLocale()) {
				this.locale = getLocale();
				this.refreshLabels();
			}
			const { x, y } = await computePosition({ getBoundingClientRect: () => rect }, this.bubble, {
				strategy: 'fixed',
				placement: 'top',
				middleware: [offset(8), flip(), shift({ padding: 8 })]
			});
			this.bubble.style.left = `${x}px`;
			this.bubble.style.top = `${y}px`;
			this.bubble.classList.add('is-visible');
		}

		update(update: ViewUpdate): void {
			if (update.docChanged || update.selectionSet) this.suppressed = false;
			if (
				update.focusChanged &&
				!update.view.hasFocus &&
				!this.bubble.contains(document.activeElement)
			) {
				this.suppressed = true;
			}
			if (
				update.docChanged ||
				update.selectionSet ||
				update.viewportChanged ||
				update.focusChanged
			) {
				this.schedule();
			}
		}

		destroy(): void {
			if (this.frame) cancelAnimationFrame(this.frame);
			window.removeEventListener('mouseup', this.onRelease);
			window.removeEventListener('scroll', this.onScroll, true);
			this.releaseEscape();
			this.bubble.remove();
		}
	},
	{
		eventHandlers: {
			mousedown(event) {
				if ((event as MouseEvent).button === 0) {
					this.dragging = true;
					this.hide();
				}
				return false;
			}
		}
	}
);
