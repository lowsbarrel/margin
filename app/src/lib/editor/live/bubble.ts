import { computePosition, flip, offset, shift } from '@floating-ui/dom';
import { syntaxTree } from '@codemirror/language';
import type { EditorState } from '@codemirror/state';
import type { SyntaxNode } from '@lezer/common';
import { ViewPlugin, type EditorView, type ViewUpdate } from '@codemirror/view';
import { mount, unmount } from 'svelte';
import SelectionToolbar from '$lib/components/editor/SelectionToolbar.svelte';
import { ToolbarState } from './bubble-state.svelte';
import { fencedRange, type BlockType } from './commands';
import { ESCAPE_BUBBLE, onEscape } from './escape';
import { activeMarks } from './marks';
import './assist.css';

const HEADS: BlockType[] = [
	'text',
	'heading1',
	'heading2',
	'heading3',
	'heading4',
	'heading5',
	'heading6'
];

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

function insideCode(state: EditorState, names: Set<string>): boolean {
	if (names.has('CodeBlock')) return true;
	const fence = fencedRange(state);
	if (!fence) return false;
	const { from, to } = state.selection.main;
	return from > fence.from || to < fence.to;
}

function blockType(state: EditorState, names: Set<string>): BlockType {
	if (fencedRange(state)) return 'code';
	for (let level = 1; level <= 6; level++) {
		if (names.has(`ATXHeading${level}`) || names.has(`SetextHeading${level}`)) return HEADS[level];
	}
	if (names.has('Task')) return 'task';
	if (names.has('BulletList')) return 'bullet';
	if (names.has('OrderedList')) return 'ordered';
	if (names.has('Blockquote')) return 'quote';
	return 'text';
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
		readonly host: HTMLDivElement;
		readonly status = new ToolbarState();
		app: Record<string, unknown> | null = null;
		frame = 0;
		dragging = false;
		suppressed = false;
		onRelease = () => {
			this.dragging = false;
			this.schedule();
		};
		releaseEscape: () => void;
		// The editor scrolls in an outer container, so CodeMirror's own scroll events never fire.
		onScroll = () => this.schedule();

		constructor(readonly view: EditorView) {
			this.host = document.createElement('div');
			this.host.className = 'cm-assist-bubble';
			document.body.appendChild(this.host);
			this.app = mount(SelectionToolbar, {
				target: this.host,
				props: { view, status: this.status }
			});
			this.host.addEventListener('mousedown', (event) => {
				if ((event.target as HTMLElement).closest('input')) return;
				event.preventDefault();
			});
			window.addEventListener('mouseup', this.onRelease);
			window.addEventListener('scroll', this.onScroll, true);
			this.releaseEscape = onEscape(view, ESCAPE_BUBBLE, () => {
				if (!this.host.classList.contains('is-visible')) return false;
				this.suppressed = true;
				this.hide();
				return true;
			});
		}

		schedule(): void {
			if (this.frame) return;
			this.frame = requestAnimationFrame(() => {
				this.frame = 0;
				void this.place();
			});
		}

		hide(): void {
			this.host.classList.remove('is-visible');
		}

		async place(): Promise<void> {
			const { view, status } = this;
			const range = view.state.selection.main;
			const names = ancestorNames(view.state);
			if (range.empty || this.dragging || this.suppressed || insideCode(view.state, names)) {
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
			status.marks = activeMarks(view.state);
			status.block = blockType(view.state, names);
			const { x, y } = await computePosition({ getBoundingClientRect: () => rect }, this.host, {
				strategy: 'fixed',
				placement: 'top',
				middleware: [offset(8), flip(), shift({ padding: 8 })]
			});
			this.host.style.left = `${x}px`;
			this.host.style.top = `${y}px`;
			this.host.classList.add('is-visible');
		}

		update(update: ViewUpdate): void {
			if (update.docChanged || update.selectionSet) this.suppressed = false;
			if (update.focusChanged) {
				if (update.view.hasFocus) this.suppressed = false;
				else if (!this.host.contains(document.activeElement)) this.suppressed = true;
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
			if (this.app) void unmount(this.app);
			this.host.remove();
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
