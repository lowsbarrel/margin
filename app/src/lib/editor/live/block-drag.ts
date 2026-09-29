import { EditorSelection, type Extension, type TransactionSpec } from '@codemirror/state';
import { ViewPlugin, type EditorView, type ViewUpdate } from '@codemirror/view';
import {
	blockNear,
	draggableAt,
	dropBeside,
	dropItem,
	listGaps,
	type Block,
	type Gap
} from './block-drop';
import { dragEnabled, gripElement, markSource, scrollParent, sourceLines } from './block-grip';
import { ESCAPE_DRAG, onEscape } from './escape';
import { ancestorAt, leadingSpace, listAt } from './list-tree';
import './block-drag.css';

const GRIP_WIDTH = 18;
const GRIP_HEIGHT = 24;
const GRIP_GAP = 6;
const TABLE_SHIFT = 22;
const DRAG_THRESHOLD = 4;
const LEVEL_STEP = 24;
const EDGE = 48;
const SCROLL_STEP = 16;

type Plan =
	| { kind: 'gap'; gap: Gap; level: number; x: number; y: number }
	| { kind: 'beside'; target: Block; after: boolean; x: number; y: number };

interface Session {
	source: Block;
	pointerId: number;
	startX: number;
	startY: number;
	x: number;
	y: number;
	moved: boolean;
	dirty: boolean;
	plan: Plan | null;
	scroller: HTMLElement | null;
	frame: number;
	release: () => void;
}

class BlockDrag {
	private readonly view: EditorView;
	private readonly grip = gripElement();
	private readonly marker = document.createElement('div');
	private hovered: Block | null = null;
	private pointer: { x: number; y: number; buttons: number } | null = null;
	private frame = 0;
	private session: Session | null = null;

	constructor(view: EditorView) {
		this.view = view;
		this.marker.className = 'cm-lp-drop-line';
		view.dom.append(this.grip, this.marker);
		view.dom.addEventListener('mousemove', this.onHover);
		view.dom.addEventListener('mouseleave', this.onLeave);
		this.grip.addEventListener('pointerdown', this.onPress);
	}

	update(update: ViewUpdate): void {
		if (update.docChanged) {
			if (this.session) queueMicrotask(() => this.finish(false));
			this.hide();
		} else if (update.geometryChanged && this.hovered) {
			this.schedule();
		}
	}

	destroy(): void {
		cancelAnimationFrame(this.frame);
		if (this.session) {
			this.session.release();
			document.body.classList.remove('cm-lp-dragging');
			this.session = null;
		}
		this.view.dom.removeEventListener('mousemove', this.onHover);
		this.view.dom.removeEventListener('mouseleave', this.onLeave);
		this.grip.remove();
		this.marker.remove();
	}

	private readonly onHover = (event: MouseEvent): void => {
		if (this.session) return;
		this.pointer = { x: event.clientX, y: event.clientY, buttons: event.buttons };
		this.schedule();
	};

	private readonly onLeave = (event: MouseEvent): void => {
		if (this.session || this.view.dom.contains(event.relatedTarget as Node | null)) return;
		this.pointer = null;
		this.hide();
	};

	private schedule(): void {
		if (!this.frame) this.frame = requestAnimationFrame(this.render);
	}

	private readonly render = (): void => {
		this.frame = 0;
		const pointer = this.pointer;
		if (this.session || !pointer || pointer.buttons || !dragEnabled(this.view)) {
			if (!this.session) this.hide();
			return;
		}
		const pos = this.lineAt(pointer.y, false);
		const block = pos === null ? null : draggableAt(this.view.state, pos);
		if (block) this.place(block);
		else this.hide();
	};

	private lineAt(y: number, clamp: boolean): number | null {
		const view = this.view;
		const top = y - view.documentTop;
		const bottom = view.lineBlockAt(view.state.doc.length).bottom;
		if (!clamp && (top < 0 || top > bottom)) return null;
		return view.lineBlockAtHeight(Math.max(0, Math.min(top, bottom - 1))).from;
	}

	private textEdges(): { left: number; right: number } {
		const content = this.view.contentDOM;
		const rect = content.getBoundingClientRect();
		const style = getComputedStyle(content);
		return {
			left: rect.left + parseFloat(style.paddingLeft),
			right: rect.right - parseFloat(style.paddingRight)
		};
	}

	private place(block: Block): void {
		const view = this.view;
		const box = view.dom.getBoundingClientRect();
		const row = view.lineBlockAt(block.from);
		const coords = view.coordsAtPos(block.anchor, 1);
		const line = view.defaultLineHeight;
		const middle =
			coords && coords.bottom - coords.top < line * 2
				? (coords.top + coords.bottom) / 2
				: view.documentTop + row.top + Math.min(row.height, line) / 2;
		const edge = block.item && coords ? coords.left : this.textEdges().left;
		const left = edge - (block.node === 'Table' ? TABLE_SHIFT : 0) - GRIP_WIDTH - GRIP_GAP;
		this.grip.style.transform = `translate(${left - box.left}px, ${middle - box.top - GRIP_HEIGHT / 2}px)`;
		this.grip.dataset.visible = '';
		this.hovered = block;
	}

	private hide(): void {
		delete this.grip.dataset.visible;
		this.hovered = null;
	}

	private readonly onPress = (event: PointerEvent): void => {
		const source = this.hovered;
		if (event.button !== 0 || !source || this.session || !dragEnabled(this.view)) return;
		event.preventDefault();
		this.grip.setPointerCapture(event.pointerId);
		const controller = new AbortController();
		const { signal } = controller;
		window.addEventListener('pointermove', this.onDrag, { signal });
		window.addEventListener('pointerup', this.onRelease, { signal });
		window.addEventListener('pointercancel', () => this.finish(false), { signal });
		window.addEventListener('blur', () => this.finish(false), { signal });
		document.addEventListener('visibilitychange', () => document.hidden && this.finish(false), {
			signal
		});
		let stopEscape: (() => void) | null = null;
		const session: Session = {
			source,
			pointerId: event.pointerId,
			startX: event.clientX,
			startY: event.clientY,
			x: event.clientX,
			y: event.clientY,
			moved: false,
			dirty: false,
			plan: null,
			scroller: null,
			frame: 0,
			release: () => {
				controller.abort();
				stopEscape?.();
				cancelAnimationFrame(session.frame);
			}
		};
		this.session = session;
		stopEscape = onEscape(this.view, ESCAPE_DRAG, () => {
			if (!this.session?.moved) return false;
			this.finish(false);
			return true;
		});
	};

	private readonly onDrag = (event: PointerEvent): void => {
		const session = this.session;
		if (!session || event.pointerId !== session.pointerId) return;
		session.x = event.clientX;
		session.y = event.clientY;
		session.dirty = true;
		if (session.moved) return;
		if (Math.hypot(session.x - session.startX, session.y - session.startY) < DRAG_THRESHOLD) return;
		session.moved = true;
		session.scroller = scrollParent(this.view.dom);
		document.body.classList.add('cm-lp-dragging');
		this.grip.dataset.dragging = '';
		this.view.dispatch({
			effects: markSource.of({ from: session.source.from, to: session.source.to })
		});
		this.view.focus();
		session.frame = requestAnimationFrame(this.loop);
	};

	private readonly onRelease = (event: PointerEvent): void => {
		if (event.pointerId === this.session?.pointerId) this.finish(true);
	};

	private readonly loop = (): void => {
		const session = this.session;
		if (!session?.moved) return;
		const scroller = session.scroller;
		if (scroller) {
			const rect = scroller.getBoundingClientRect();
			const above = session.y - rect.top;
			const below = rect.bottom - session.y;
			let step = 0;
			if (above < EDGE) step = -SCROLL_STEP * (1 - Math.max(0, above) / EDGE);
			else if (below < EDGE) step = SCROLL_STEP * (1 - Math.max(0, below) / EDGE);
			const before = scroller.scrollTop;
			if (step) scroller.scrollTop += step;
			if (scroller.scrollTop !== before) session.dirty = true;
		}
		if (session.dirty) {
			session.dirty = false;
			session.plan = this.planAt(session);
			this.drawPlan(session.plan);
		}
		session.frame = requestAnimationFrame(this.loop);
	};

	private drawPlan(plan: Plan | null): void {
		if (!plan) {
			delete this.marker.dataset.visible;
			return;
		}
		const box = this.view.dom.getBoundingClientRect();
		const width = Math.max(GRIP_HEIGHT, this.textEdges().right - plan.x);
		this.marker.style.transform = `translate(${plan.x - box.left}px, ${plan.y - box.top}px)`;
		this.marker.style.width = `${width}px`;
		this.marker.dataset.visible = '';
	}

	private planAt(session: Session): Plan | null {
		const { view } = this;
		const state = view.state;
		const pos = this.lineAt(session.y, true);
		if (pos === null) return null;
		const source = session.source;
		if (source.item) {
			const line = state.doc.lineAt(pos);
			const list = listAt(state, line.from + leadingSpace(line.text).length);
			let best: Gap | null = null;
			let bestY = 0;
			for (const gap of list ? listGaps(state.doc, list, source.item) : []) {
				const row = view.lineBlockAt(gap.pos);
				const y = view.documentTop + (gap.after ? row.bottom : row.top);
				if (!best || Math.abs(y - session.y) < Math.abs(bestY - session.y)) {
					best = gap;
					bestY = y;
				}
			}
			if (best) {
				const want = source.item.level + Math.round((session.x - session.startX) / LEVEL_STEP);
				const level = Math.max(best.min, Math.min(best.max, want));
				return { kind: 'gap', gap: best, level, x: this.levelX(best, level), y: bestY };
			}
		}
		const near = blockNear(state, pos, false);
		if (!near) return null;
		const target = near.block;
		if (target.from <= source.to && source.from <= target.to) return null;
		const top = view.documentTop + view.lineBlockAt(target.from).top;
		const bottom = view.documentTop + view.lineBlockAt(target.to).bottom;
		const after = target.node === 'Frontmatter' || (near.after ?? session.y > (top + bottom) / 2);
		return { kind: 'beside', target, after, x: this.textEdges().left, y: after ? bottom : top };
	}

	private levelX(gap: Gap, level: number): number {
		const ref =
			gap.next && gap.next.level === level
				? gap.next
				: gap.prev && level <= gap.prev.level
					? ancestorAt(gap.prev, level)
					: null;
		const anchor = ref ?? gap.prev;
		const coords = anchor ? this.view.coordsAtPos(anchor.mark, 1) : null;
		if (!coords) return this.textEdges().left + (level - 1) * LEVEL_STEP;
		return ref ? coords.left : coords.left + LEVEL_STEP;
	}

	private specFor(source: Block, plan: Plan): TransactionSpec | null {
		const state = this.view.state;
		if (plan.kind === 'gap')
			return source.item ? dropItem(state, source.item, plan.gap, plan.level) : null;
		return dropBeside(state, source, plan.target, plan.after);
	}

	private finish(apply: boolean): void {
		const session = this.session;
		if (!session) return;
		this.session = null;
		session.release();
		document.body.classList.remove('cm-lp-dragging');
		delete this.grip.dataset.dragging;
		delete this.marker.dataset.visible;
		const view = this.view;
		if (!session.moved) {
			if (!apply) return;
			view.dispatch({ selection: EditorSelection.range(session.source.from, session.source.to) });
			view.focus();
			return;
		}
		const plan = apply ? this.planAt(session) : null;
		const spec = plan && this.specFor(session.source, plan);
		const effects = markSource.of(null);
		view.dispatch(spec ? { ...spec, effects, userEvent: 'move.drop' } : { effects });
		this.hide();
	}
}

export const blockDrag: Extension = [sourceLines, ViewPlugin.fromClass(BlockDrag)];
