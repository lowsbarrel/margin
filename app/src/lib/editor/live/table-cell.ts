import { redo, standardKeymap, undo } from '@codemirror/commands';
import {
	Annotation,
	EditorSelection,
	EditorState,
	Prec,
	Transaction,
	type ChangeSpec,
	type Extension,
	type TransactionSpec
} from '@codemirror/state';
import {
	EditorView,
	ViewPlugin,
	drawSelection,
	keymap,
	type Rect,
	type ViewUpdate
} from '@codemirror/view';
import { minimalDiff } from '$lib/utils/text-diff';
import { autoPair } from './autopair';
import { liveClicks } from './click';
import { insertLink } from './commands';
import { cellAssist } from './complete';
import { contextOf, liveContext } from './context';
import { ESCAPE_TABLE, onEscape } from './escape';
import { liveInlineHtml } from './html-inline';
import { MARKER, toggleMark, type MarkKind } from './marks';
import { liveMath } from './math';
import { livePaste } from './paste';
import { livePreview } from './preview';
import { freezeReveal, liveReveal } from './reveal';
import { cellSyntax } from './syntax';
import { cellWidgets } from './table-cell-widgets';
import { retypeCell } from './table-format';
import { tableAt } from './table-model';
import {
	enterCell,
	horizontalCell,
	leaveTable,
	locate,
	nextCell,
	previousCell,
	verticalCell
} from './table-nav';
import { clearCells, copyCells, dropRange, formatCells, selectTable } from './table-range';
import { liveTheme } from './theme';

const syncCell = Annotation.define<boolean>();
const UNESCAPED_PIPE = /(?<!\\)((?:\\\\)*)\|/g;

interface Session {
	sub: EditorView;
	host: HTMLElement;
	row: number;
	col: number;
	from: number;
	release: () => void;
}

const sessions = new WeakMap<EditorView, Session>();
const handoff = new WeakSet<EditorView>();
const closedRow = new WeakSet<EditorView>();

function moved(selection: EditorSelection, by: number, max = Infinity): EditorSelection {
	const fit = (pos: number) => Math.max(0, Math.min(max, pos + by));
	return EditorSelection.create(
		selection.ranges.map((range) => EditorSelection.range(fit(range.anchor), fit(range.head))),
		selection.mainIndex
	);
}

const keepInCell = EditorState.transactionFilter.of((tr) => {
	if (!tr.docChanged || tr.annotation(syncCell)) return tr;
	const specs: ChangeSpec[] = [];
	let rewritten = false;
	tr.changes.iterChanges((fromA, toA, _fromB, _toB, inserted) => {
		const text = inserted.toString();
		const clean = text.replace(/\r?\n/g, ' ').replace(UNESCAPED_PIPE, '$1\\|');
		if (clean !== text) rewritten = true;
		specs.push({ from: fromA, to: toA, insert: clean });
	});
	if (!rewritten) return tr;
	const changes = tr.startState.changes(specs);
	const userEvent = tr.annotation(Transaction.userEvent);
	return {
		changes,
		selection: tr.startState.selection.map(changes, 1),
		effects: tr.effects,
		scrollIntoView: tr.scrollIntoView,
		annotations: userEvent ? [Transaction.userEvent.of(userEvent)] : []
	};
});

function stepVertically(main: EditorView, sub: EditorView, forward: boolean): boolean {
	const range = sub.state.selection.main;
	const target = sub.moveVertically(range, forward);
	const here = sub.coordsAtPos(range.head);
	const there = sub.coordsAtPos(target.head);
	const wrapsWithin =
		target.head !== range.head && here && there && Math.abs(there.top - here.top) > 2;
	if (wrapsWithin) return false;
	return verticalCell(main, forward ? 1 : -1);
}

function stepAcross(main: EditorView, sub: EditorView, dir: -1 | 1): boolean {
	const range = sub.state.selection.main;
	if (!range.empty || range.head !== (dir > 0 ? sub.state.doc.length : 0)) return false;
	return horizontalCell(main, dir);
}

function insertBreak(sub: EditorView): boolean {
	sub.dispatch(sub.state.replaceSelection('<br>'), { userEvent: 'input', scrollIntoView: true });
	return true;
}

function typePipe(main: EditorView, sub: EditorView): boolean {
	const range = sub.state.selection.main;
	const text = sub.state.doc.toString();
	if (!range.empty || text.slice(range.head).trim() || text[range.head - 1] === '\\') return false;
	const at = locate(main);
	if (!at) return false;
	if (at.col === 0 && !text) return true;
	if (at.col + 1 < at.table.cols) return afterMove(main, nextCell(main));
	closedRow.add(sub);
	return true;
}

// WebKit only lets script move focus into an editor during the input event itself, not a frame later.
export function focusEditor(main: EditorView): void {
	const session = sessions.get(main);
	if (session?.sub.dom.isConnected) session.sub.focus();
	else main.focus();
}

export function pressCell(main: EditorView, event: MouseEvent): void {
	const session = sessions.get(main);
	if (!session?.sub.dom.isConnected) return;
	const { sub } = session;
	if (event.button === 0) sub.dispatch({ effects: freezeReveal.of([]) });
	const { clientX, clientY, screenX, screenY, button, buttons, detail } = event;
	const { altKey, ctrlKey, metaKey, shiftKey } = event;
	const init = { clientX, clientY, screenX, screenY, button, buttons, detail, cancelable: true };
	const press = new MouseEvent('mousedown', {
		...init,
		altKey,
		ctrlKey,
		metaKey,
		shiftKey,
		view: window
	});
	sub.contentDOM.dispatchEvent(press);
}

export function endPress(main: EditorView): void {
	const session = sessions.get(main);
	if (!session) return;
	const { sub } = session;
	sub.contentDOM.ownerDocument.dispatchEvent(new MouseEvent('mouseup', { view: window }));
	sub.dispatch({ selection: { anchor: sub.state.selection.main.head } });
}

export function editingView(main: EditorView): EditorView {
	return sessions.get(main)?.sub ?? main;
}

export function afterMove(main: EditorView, moved: boolean): boolean {
	if (moved) focusEditor(main);
	return moved;
}

function copyCell(sub: EditorView, event: ClipboardEvent, cut: boolean): boolean {
	if (!sub.state.selection.main.empty || !event.clipboardData) return false;
	event.clipboardData.setData('text/plain', sub.state.doc.toString());
	event.preventDefault();
	if (cut)
		sub.dispatch({ changes: { from: 0, to: sub.state.doc.length }, userEvent: 'delete.cut' });
	return true;
}

function selectWholeTable(main: EditorView, event: MouseEvent): boolean {
	const session = sessions.get(main);
	const table = session && tableAt(main.state, session.from);
	if (!table) return false;
	event.preventDefault();
	selectTable(main, table);
	return true;
}

export function joinsCellHistory(tr: Transaction): boolean {
	return tr.annotation(syncCell) === true;
}

function cellKeymap(main: EditorView) {
	const mark = (kind: MarkKind) => ({
		run: (sub: EditorView) => formatCells(main, MARKER[kind]) || toggleMark(sub, kind),
		preventDefault: true
	});
	return Prec.highest(
		keymap.of([
			{ key: 'Tab', run: () => afterMove(main, nextCell(main)) },
			{ key: 'Shift-Tab', run: () => afterMove(main, previousCell(main)) },
			{
				key: 'Enter',
				run: (sub) => afterMove(main, enterCell(main, closedRow.has(sub) ? 0 : null))
			},
			{ key: 'Shift-Enter', run: insertBreak },
			{ key: '|', run: (sub) => typePipe(main, sub) },
			{ key: 'ArrowDown', run: (sub) => afterMove(main, stepVertically(main, sub, true)) },
			{ key: 'ArrowUp', run: (sub) => afterMove(main, stepVertically(main, sub, false)) },
			{ key: 'ArrowRight', run: (sub) => afterMove(main, stepAcross(main, sub, 1)) },
			{ key: 'ArrowLeft', run: (sub) => afterMove(main, stepAcross(main, sub, -1)) },
			{ key: 'Backspace', run: () => clearCells(main) },
			{ key: 'Delete', run: () => clearCells(main) },
			{ key: 'Mod-z', run: () => undo(main), preventDefault: true },
			{ key: 'Mod-Shift-z', run: () => redo(main), preventDefault: true },
			{ key: 'Mod-y', run: () => redo(main), preventDefault: true },
			{ key: 'Mod-b', ...mark('bold') },
			{ key: 'Mod-i', ...mark('italic') },
			{ key: 'Mod-e', ...mark('code') },
			{ key: 'Mod-Shift-x', ...mark('strike') },
			{ key: 'Mod-Shift-h', ...mark('highlight') },
			{ key: 'Mod-k', run: insertLink, preventDefault: true }
		])
	);
}

function cellExtensions(main: EditorView): Extension[] {
	return [
		liveContext.of(contextOf(main.state)),
		cellSyntax,
		liveTheme,
		liveReveal,
		livePreview,
		liveMath,
		liveClicks,
		cellAssist,
		livePaste,
		autoPair,
		cellWidgets,
		liveInlineHtml,
		drawSelection(),
		EditorView.domEventHandlers({
			copy: (event, sub) => copyCells(main, event, false) || copyCell(sub, event, false),
			cut: (event, sub) => copyCells(main, event, true) || copyCell(sub, event, true),
			mousedown: (event) => event.detail >= 4 && selectWholeTable(main, event)
		}),
		keepInCell,
		cellKeymap(main),
		keymap.of(standardKeymap),
		EditorView.lineWrapping,
		EditorView.contentAttributes.of({ 'aria-multiline': 'false' })
	];
}

function forwardToMain(main: EditorView) {
	return (trs: readonly Transaction[], sub: EditorView): void => {
		sub.update(trs);
		closedRow.delete(sub);
		const session = sessions.get(main);
		if (session?.sub !== sub) return;
		for (const tr of trs) {
			if (tr.annotation(syncCell) || (!tr.docChanged && !tr.selection)) continue;
			const { from } = session;
			const retyped = tr.docChanged
				? retypeCell(main.state, from, session, tr.newDoc.toString())
				: null;
			const changes: ChangeSpec[] = retyped?.changes ?? [];
			if (!retyped)
				tr.changes.iterChanges((fromA, toA, _fromB, _toB, insert) =>
					changes.push({ from: fromA + from, to: toA + from, insert })
				);
			if (retyped) session.from = retyped.from;
			// A `select.pointer` transaction makes the main view write the DOM selection, taking focus from the cell.
			const userEvent = tr.isUserEvent('select.pointer')
				? 'select'
				: tr.annotation(Transaction.userEvent);
			main.dispatch({
				changes,
				selection: tr.selection || retyped ? moved(tr.newSelection, session.from) : undefined,
				annotations: [
					syncCell.of(true),
					...(userEvent ? [Transaction.userEvent.of(userEvent)] : [])
				]
			});
		}
	};
}

function follow(session: Session, tr: Transaction): void {
	const { sub } = session;
	let from = tr.changes.mapPos(session.from, -1);
	let to = Math.max(from, tr.changes.mapPos(session.from + sub.state.doc.length, 1));
	const touched = tr.changes.touchesRange(session.from, session.from + sub.state.doc.length);
	const cell = touched ? tableAt(tr.state, from)?.rows[session.row]?.cells[session.col] : null;
	if (cell) ({ from, to } = cell);
	session.from = from;
	const text = tr.state.sliceDoc(from, to);
	const current = sub.state.doc.toString();
	const spec: TransactionSpec = { annotations: syncCell.of(true) };
	if (text !== current) spec.changes = minimalDiff(current, text);
	const head = tr.newSelection.main.head;
	if (tr.selection && head >= from && head <= to)
		spec.selection = moved(tr.newSelection, -from, text.length);
	if (spec.changes || spec.selection) sub.dispatch(spec);
}

export function cellSession(main: EditorView): Session | undefined {
	return sessions.get(main);
}

export function cellCoords(main: EditorView, pos: number, side: -1 | 1): Rect | null {
	const session = sessions.get(main);
	if (!session) return null;
	const at = pos - session.from;
	if (at < 0 || at > session.sub.state.doc.length) return null;
	return session.sub.coordsAtPos(at, side);
}

export function unmountCell(main: EditorView): void {
	const session = sessions.get(main);
	if (!session) return;
	sessions.delete(main);
	const hadFocus = session.host.contains(document.activeElement);
	session.release();
	session.sub.destroy();
	if (!hadFocus) return;
	handoff.add(main);
	queueMicrotask(() => {
		handoff.delete(main);
		if (!sessions.has(main) && main.dom.isConnected) main.focus();
	});
}

export function mountCell(
	main: EditorView,
	host: HTMLElement,
	place: { row: number; col: number; from: number; to: number }
): Session {
	const previous = sessions.get(main);
	const wantsFocus =
		main.hasFocus ||
		handoff.has(main) ||
		(!!previous && previous.host.contains(document.activeElement));
	unmountCell(main);
	const doc = main.state.sliceDoc(place.from, place.to);
	const sub = new EditorView({
		parent: host,
		state: EditorState.create({
			doc,
			selection: moved(main.state.selection, -place.from, doc.length),
			extensions: cellExtensions(main)
		}),
		dispatchTransactions: forwardToMain(main)
	});
	const session: Session = {
		sub,
		host,
		row: place.row,
		col: place.col,
		from: place.from,
		release: onEscape(sub, ESCAPE_TABLE, () => dropRange(main) || leaveTable(main))
	};
	sessions.set(main, session);
	if (!wantsFocus) return session;
	if (host.isConnected) sub.focus();
	else
		requestAnimationFrame(() => {
			if (sessions.get(main) === session && !sub.hasFocus) sub.focus();
		});
	return session;
}

export const cellSync = ViewPlugin.fromClass(
	class {
		readonly view: EditorView;

		constructor(view: EditorView) {
			this.view = view;
		}

		update(update: ViewUpdate): void {
			const session = sessions.get(update.view);
			if (!session) return;
			for (const tr of update.transactions) {
				if (tr.annotation(syncCell) || (!tr.docChanged && !tr.selection)) continue;
				follow(session, tr);
			}
		}

		destroy(): void {
			unmountCell(this.view);
		}
	}
);
