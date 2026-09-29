import { EditorSelection, type SelectionRange } from '@codemirror/state';
import { EditorView, type MouseSelectionStyle } from '@codemirror/view';
import { outsideHidden } from './hidden-marks';
import { livePreview } from './preview';

function removeRangeAround(selection: EditorSelection, pos: number): EditorSelection | null {
	const index = selection.ranges.findIndex((range) => range.from <= pos && range.to >= pos);
	if (index < 0) return null;
	const ranges = selection.ranges.filter((_, i) => i !== index);
	const main =
		selection.mainIndex === index ? 0 : selection.mainIndex - (selection.mainIndex > index ? 1 : 0);
	return EditorSelection.create(ranges, main);
}

// A single click lands outside hidden formatting and a triple click takes the line without its break.
function clickStyle(view: EditorView, event: MouseEvent): MouseSelectionStyle | null {
	const type = event.detail;
	if (event.button !== 0 || (type !== 1 && type !== 3)) return null;
	const snap = type === 1 && view.plugin(livePreview) !== null;
	const at = (e: MouseEvent) => {
		const found = view.posAndSideAtCoords({ x: e.clientX, y: e.clientY }, false);
		return snap ? { pos: outsideHidden(view.state, found.pos), assoc: found.assoc } : found;
	};
	const rangeAt = (pos: number, assoc: number): SelectionRange => {
		if (type === 1) return EditorSelection.cursor(pos, assoc);
		const line = view.state.doc.lineAt(pos);
		return EditorSelection.range(line.from, line.to);
	};
	const start = at(event);
	let startSelection = view.state.selection;
	return {
		update(update) {
			if (!update.docChanged) return;
			start.pos = update.changes.mapPos(start.pos);
			startSelection = startSelection.map(update.changes);
		},
		get(current, extend, multiple) {
			const cur = at(current);
			let range = rangeAt(cur.pos, cur.assoc);
			if (start.pos !== cur.pos && !extend) {
				const first = rangeAt(start.pos, start.assoc);
				const from = Math.min(first.from, range.from);
				const to = Math.max(first.to, range.to);
				range =
					from < range.from ? EditorSelection.range(from, to) : EditorSelection.range(to, from);
			}
			if (extend)
				return startSelection.replaceRange(startSelection.main.extend(range.from, range.to));
			const removed =
				multiple && type === 1 && startSelection.ranges.length > 1
					? removeRangeAround(startSelection, cur.pos)
					: null;
			if (removed) return removed;
			if (multiple) return startSelection.addRange(range);
			return EditorSelection.create([range]);
		}
	};
}

export const liveMouse = [
	EditorView.mouseSelectionStyle.of(clickStyle),
	EditorView.clickAddsSelectionRange.of((event) => event.altKey)
];
