import { PluginKey, TextSelection } from '@tiptap/pm/state';
import { dropPoint } from '@tiptap/pm/transform';
import type { EditorView } from '@tiptap/pm/view';

const THRESHOLD = 5;
const SCROLL_THRESHOLD = 80;
const SCROLL_SPEED = 15;

export interface DragState {
	dropPos: number | null;
}

export const contentDragKey = new PluginKey<DragState>('contentDrag');

export function startDrag(
	view: EditorView,
	initEvent: MouseEvent,
	srcFrom: number,
	srcTo: number,
	isNode: boolean
) {
	const startX = initEvent.clientX;
	const startY = initEvent.clientY;
	let dragging = false;
	let lastDropPos: number | null = null;

	const slice = view.state.doc.slice(srcFrom, srcTo);
	let rafScheduled = false;
	let latestMoveEvent: MouseEvent | null = null;

	const scrollParent = view.dom.closest('.editor-container') || view.dom.parentElement;

	const flushMove = () => {
		rafScheduled = false;
		const e = latestMoveEvent;
		if (!e) return;

		const dx = e.clientX - startX;
		const dy = e.clientY - startY;

		if (!dragging && Math.abs(dx) + Math.abs(dy) < THRESHOLD) return;

		if (!dragging) {
			dragging = true;
			document.body.classList.add('content-dragging');
		}

		if (scrollParent) {
			const rect = scrollParent.getBoundingClientRect();
			if (e.clientY < rect.top + SCROLL_THRESHOLD) {
				scrollParent.scrollTop -= SCROLL_SPEED;
			} else if (e.clientY > rect.bottom - SCROLL_THRESHOLD) {
				scrollParent.scrollTop += SCROLL_SPEED;
			}
		}

		const result = view.posAtCoords({
			left: e.clientX,
			top: e.clientY
		});
		if (result) {
			let dp = result.pos;
			if (isNode) {
				const best = dropPoint(view.state.doc, dp, slice);
				if (best != null) dp = best;
			}
			dp = Math.max(0, Math.min(dp, view.state.doc.content.size));
			lastDropPos = dp;
		}

		const tr = view.state.tr;
		tr.setMeta(contentDragKey, { dropPos: lastDropPos } satisfies DragState);
		view.dispatch(tr);
	};

	const onMove = (e: MouseEvent) => {
		latestMoveEvent = e;
		if (!rafScheduled) {
			rafScheduled = true;
			requestAnimationFrame(flushMove);
		}
	};

	const onUp = (e: MouseEvent) => {
		window.removeEventListener('mousemove', onMove, true);
		window.removeEventListener('mouseup', onUp, true);
		document.body.classList.remove('content-dragging');

		const clearTr = view.state.tr;
		clearTr.setMeta(contentDragKey, { dropPos: null } satisfies DragState);

		if (!dragging) {
			if (!isNode) {
				const clickPos = view.posAtCoords({
					left: e.clientX,
					top: e.clientY
				});
				if (clickPos) {
					try {
						clearTr.setSelection(TextSelection.create(clearTr.doc, clickPos.pos));
					} catch {}
				}
			}
			view.dispatch(clearTr);
			view.focus();
			return;
		}

		if (lastDropPos == null) {
			view.dispatch(clearTr);
			return;
		}

		if (lastDropPos >= srcFrom && lastDropPos <= srcTo) {
			view.dispatch(clearTr);
			return;
		}

		const tr = view.state.tr;
		tr.setMeta(contentDragKey, { dropPos: null } satisfies DragState);

		let insertAt = lastDropPos;

		if (insertAt <= srcFrom) {
			tr.insert(insertAt, slice.content);
			const shift = slice.content.size;
			tr.delete(srcFrom + shift, srcTo + shift);
		} else if (insertAt >= srcTo) {
			tr.delete(srcFrom, srcTo);
			insertAt -= srcTo - srcFrom;
			const safePos = Math.min(insertAt, tr.doc.content.size);
			tr.insert(safePos, slice.content);
		} else {
			view.dispatch(clearTr);
			return;
		}

		view.dispatch(tr.setMeta('uiEvent', 'drop'));
		view.focus();
	};

	window.addEventListener('mousemove', onMove, true);
	window.addEventListener('mouseup', onUp, true);
}
