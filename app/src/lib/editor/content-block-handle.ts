import { NodeSelection, Plugin, PluginKey, TextSelection } from '@tiptap/pm/state';
import type { EditorView } from '@tiptap/pm/view';

import { startDrag } from './content-drag-move';

const handleKey = new PluginKey('blockHandle');

const HANDLE_SVG = `<svg width="8" height="14" viewBox="0 0 8 14" fill="currentColor" xmlns="http://www.w3.org/2000/svg"><circle cx="1.5" cy="2" r="1.2"/><circle cx="6.5" cy="2" r="1.2"/><circle cx="1.5" cy="7" r="1.2"/><circle cx="6.5" cy="7" r="1.2"/><circle cx="1.5" cy="12" r="1.2"/><circle cx="6.5" cy="12" r="1.2"/></svg>`;

const BLOCK_SELECTORS =
	'li, p:not(:first-child), pre, blockquote, h1, h2, h3, h4, h5, h6, ' +
	'[data-type="callout"], [data-type="mathBlock"], .file-embed, ' +
	'table, hr';

function findBlock(editorDom: HTMLElement, view: EditorView, x: number, y: number) {
	const elements = document.elementsFromPoint(x, y);
	for (const el of elements) {
		if (!editorDom.contains(el) || el === editorDom) continue;

		const blockEl = (el as HTMLElement).closest(BLOCK_SELECTORS);
		if (!blockEl || !editorDom.contains(blockEl)) continue;

		if (blockEl.classList.contains('block-drag-handle')) continue;

		try {
			const pos = view.posAtDOM(blockEl, 0);
			const $pos = view.state.doc.resolve(pos);

			for (let d = $pos.depth; d >= 1; d--) {
				const node = $pos.node(d);
				const nodePos = $pos.before(d);
				const dom = view.nodeDOM(nodePos);
				if (!dom || !(dom instanceof HTMLElement)) continue;

				if (dom === blockEl || dom.contains(blockEl)) {
					return { pos: nodePos, node, dom };
				}
			}

			const after = $pos.nodeAfter;
			if (after) {
				const dom = view.nodeDOM(pos);
				if (dom instanceof HTMLElement) {
					return { pos, node: after, dom };
				}
			}
		} catch {
			continue;
		}
	}

	const coords = view.posAtCoords({ left: x, top: y });
	if (!coords) return null;

	const $pos = view.state.doc.resolve(coords.pos);
	if ($pos.depth === 0) {
		const idx = $pos.index(0);
		if (idx >= view.state.doc.childCount) return null;
		let pos = 0;
		for (let i = 0; i < idx; i++) pos += view.state.doc.child(i).nodeSize;
		const node = view.state.doc.child(idx);
		const dom = view.nodeDOM(pos);
		if (!dom || !(dom instanceof HTMLElement)) return null;
		return { pos, node, dom };
	}

	const topPos = $pos.before(1);
	const topNode = $pos.node(1);
	const dom = view.nodeDOM(topPos);
	if (!dom || !(dom instanceof HTMLElement)) return null;
	return { pos: topPos, node: topNode, dom };
}

export function createBlockHandlePlugin(): Plugin {
	return new Plugin({
		key: handleKey,

		view(editorView) {
			const handle = document.createElement('div');
			handle.className = 'block-drag-handle';
			handle.contentEditable = 'false';
			handle.setAttribute('draggable', 'false');
			handle.innerHTML = HANDLE_SVG;

			const editorDom = editorView.dom;
			const wrap = editorDom.parentElement!;
			wrap.style.position = 'relative';
			wrap.appendChild(handle);

			let activeNodePos: number | null = null;
			let activeNodeEnd = 0;
			let hoveredDom: HTMLElement | null = null;
			let isHandleHovered = false;

			function positionHandle(blockDom: HTMLElement) {
				const wrapRect = wrap.getBoundingClientRect();
				const blockRect = blockDom.getBoundingClientRect();

				handle.style.display = 'flex';
				handle.style.top = `${blockRect.top - wrapRect.top}px`;
				handle.style.left = `${blockRect.left - wrapRect.left - 24}px`;
			}

			function hideHandle() {
				if (isHandleHovered) return;
				handle.style.display = '';
				activeNodePos = null;
				hoveredDom = null;
			}

			let hoverRafScheduled = false;
			let hoverRafId = 0;
			let latestHoverEvent: MouseEvent | null = null;

			const flushHover = () => {
				hoverRafScheduled = false;
				const e = latestHoverEvent;
				if (!e) return;

				if (document.body.classList.contains('content-dragging')) return;

				const result = findBlock(editorDom, editorView, e.clientX, e.clientY);
				if (!result) {
					if (!isHandleHovered) hideHandle();
					return;
				}

				if (result.dom === hoveredDom) return;

				hoveredDom = result.dom;
				activeNodePos = result.pos;
				activeNodeEnd = result.pos + result.node.nodeSize;
				positionHandle(result.dom);
			};

			const onEditorMouseMove = (e: MouseEvent) => {
				latestHoverEvent = e;
				if (!hoverRafScheduled) {
					hoverRafScheduled = true;
					hoverRafId = requestAnimationFrame(flushHover);
				}
			};

			const onEditorMouseLeave = () => {
				setTimeout(() => {
					if (!isHandleHovered) hideHandle();
				}, 80);
			};

			handle.addEventListener('mouseenter', () => {
				isHandleHovered = true;
			});
			handle.addEventListener('mouseleave', () => {
				isHandleHovered = false;
				if (!editorDom.matches(':hover')) hideHandle();
			});

			handle.addEventListener('mousedown', (e: MouseEvent) => {
				e.preventDefault();
				e.stopPropagation();
				if (activeNodePos == null) return;

				const from = activeNodePos;
				const to = activeNodeEnd;

				try {
					const tr = editorView.state.tr.setSelection(
						NodeSelection.create(editorView.state.doc, from)
					);
					editorView.dispatch(tr);
				} catch {
					try {
						const tr = editorView.state.tr.setSelection(
							TextSelection.create(editorView.state.doc, from + 1, to - 1)
						);
						editorView.dispatch(tr);
					} catch {}
				}

				editorView.focus();
				startDrag(editorView, e, from, to, true);
			});

			const scrollParent = wrap.closest('.editor-container');
			const onScroll = () => hideHandle();

			editorDom.addEventListener('mousemove', onEditorMouseMove);
			editorDom.addEventListener('mouseleave', onEditorMouseLeave);
			scrollParent?.addEventListener('scroll', onScroll, { passive: true });

			const onKeyDown = () => hideHandle();
			editorDom.addEventListener('keydown', onKeyDown);

			const onWheel = () => hideHandle();
			editorDom.addEventListener('wheel', onWheel, { passive: true });

			return {
				update() {
					if (hoveredDom && activeNodePos != null) {
						if (document.body.contains(hoveredDom)) {
							positionHandle(hoveredDom);
						} else {
							hideHandle();
						}
					}
				},
				destroy() {
					if (hoverRafScheduled) {
						cancelAnimationFrame(hoverRafId);
						hoverRafScheduled = false;
					}
					editorDom.removeEventListener('mousemove', onEditorMouseMove);
					editorDom.removeEventListener('mouseleave', onEditorMouseLeave);
					editorDom.removeEventListener('keydown', onKeyDown);
					editorDom.removeEventListener('wheel', onWheel);
					scrollParent?.removeEventListener('scroll', onScroll);
					handle.remove();
				}
			};
		}
	});
}
