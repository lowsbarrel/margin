import { simplifySelection } from '@codemirror/commands';
import { EditorSelection, type Line, type SelectionRange } from '@codemirror/state';
import { Direction, ViewPlugin, keymap, type EditorView } from '@codemirror/view';
import { calloutAt } from './callouts';
import { frontmatterEnd } from './decorate';
import { ESCAPE_SELECTION, onEscape } from './escape';
import { coveredByHidden } from './hidden-marks';

// A step over hidden formatting alone stops at its edge, where the caret reveals it, before crossing it.
function groupHead(view: EditorView, range: SelectionRange, forward: boolean): number {
	const target = view.moveByGroup(range, forward).head;
	const { doc } = view.state;
	let at = range.head;
	while (at !== target && /\s/.test(doc.sliceString(forward ? at : at - 1, forward ? at + 1 : at)))
		at += forward ? 1 : -1;
	if (at === range.head || at === target) return target;
	return coveredByHidden(view.state, Math.min(at, target), Math.max(at, target)) ? at : target;
}

function moveGroup(view: EditorView, right: boolean, extend: boolean): boolean {
	const { state } = view;
	const selection = EditorSelection.create(
		state.selection.ranges.map((range) => {
			const forward = right === (view.textDirectionAt(range.head) === Direction.LTR);
			if (!extend && !range.empty) return EditorSelection.cursor(forward ? range.to : range.from);
			const head = groupHead(view, range, forward);
			return extend ? EditorSelection.range(range.anchor, head) : EditorSelection.cursor(head);
		}),
		state.selection.mainIndex
	);
	if (selection.eq(state.selection)) return false;
	view.dispatch({ selection, scrollIntoView: true, userEvent: 'select' });
	return true;
}

function collapsed(view: EditorView, line: Line): boolean {
	if (!line.length || line.from < view.viewport.from || line.to > view.viewport.to) return false;
	const { node } = view.domAtPos(line.from);
	const row = (node instanceof Element ? node : node.parentElement)?.closest('.cm-line');
	return !row || row.getBoundingClientRect().height < 1;
}

function entersBlock(view: EditorView, from: Line, to: Line): boolean {
	const { state } = view;
	if (to.to <= frontmatterEnd(state)) return false;
	const callout = calloutAt(state, to.from);
	if (callout !== null) return callout !== calloutAt(state, from.from);
	return collapsed(view, to);
}

// Arrowing into a rendered block lands at the start of its nearest source line, dropping the goal column.
function enterBlock(view: EditorView, forward: boolean, extend: boolean): boolean {
	const { state } = view;
	const range = state.selection.main;
	if (state.selection.ranges.length > 1 || (!extend && !range.empty)) return false;
	const { doc } = state;
	const line = doc.lineAt(range.head);
	const number = line.number + (forward ? 1 : -1);
	if (number < 1 || number > doc.lines) return false;
	if (doc.lineAt(view.moveVertically(range, forward).head).number === line.number) return false;
	const next = doc.line(number);
	if (!entersBlock(view, line, next)) return false;
	view.dispatch({
		selection: extend
			? EditorSelection.range(range.anchor, next.from)
			: EditorSelection.cursor(next.from),
		scrollIntoView: true,
		userEvent: 'select'
	});
	return true;
}

export const liveMotion = keymap.of([
	{
		key: 'Mod-ArrowLeft',
		mac: 'Alt-ArrowLeft',
		run: (view) => moveGroup(view, false, false),
		shift: (view) => moveGroup(view, false, true),
		preventDefault: true
	},
	{
		key: 'Mod-ArrowRight',
		mac: 'Alt-ArrowRight',
		run: (view) => moveGroup(view, true, false),
		shift: (view) => moveGroup(view, true, true),
		preventDefault: true
	},
	{
		key: 'ArrowDown',
		run: (view) => enterBlock(view, true, false),
		shift: (view) => enterBlock(view, true, true)
	},
	{
		key: 'ArrowUp',
		run: (view) => enterBlock(view, false, false),
		shift: (view) => enterBlock(view, false, true)
	}
]);

export const selectionEscape = ViewPlugin.define((view) => ({
	destroy: onEscape(
		view,
		ESCAPE_SELECTION,
		() => view.state.selection.ranges.length > 1 && simplifySelection(view)
	)
}));
