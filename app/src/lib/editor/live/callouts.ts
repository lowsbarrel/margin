import { syntaxTree } from '@codemirror/language';
import type { SyntaxNode } from '@lezer/common';
import {
	StateEffect,
	StateField,
	type EditorState,
	type Extension,
	type Line,
	type Range
} from '@codemirror/state';
import {
	Decoration,
	EditorView,
	ViewPlugin,
	WidgetType,
	type DecorationSet,
	type ViewUpdate
} from '@codemirror/view';
import * as m from '$lib/paraglide/messages.js';
import {
	CALLOUT_LABELS,
	calloutIcon,
	calloutKind,
	chevronElement,
	type CalloutKind
} from './callout-types';
import { hide, line, mark, refreshDecorations, touched, touchedLines } from './decorate';
import { COLON_CALLOUT } from './syntax';

interface Callout {
	kind: CalloutKind;
	header: Line;
	last: Line;
	fold: '+' | '-' | null;
	markerFrom: number;
	markerTo: number;
	title: string;
	bodyTo: number;
	close: Line | null;
	dropFrom: number | null;
	dropTo: number | null;
}

const OBSIDIAN = /^[ \t]*(?:>[ \t]*)+\[!([A-Za-z][\w-]*)\]([+-]?)[ \t]?/;
const COLON_OPEN = /^:::([A-Za-z][\w-]*)/;
const COLON_CLOSE = /^:::[ \t]*$/;

export const setCalloutFold = StateEffect.define<{ pos: number; collapsed: boolean }>();

// Fold state is view-only, so it lives in a state field and never reaches the file.
const calloutFold = StateField.define<Map<number, boolean>>({
	create: () => new Map(),
	update(value, tr) {
		let next = value;
		if (tr.docChanged) {
			next = new Map();
			for (const [pos, collapsed] of value) next.set(tr.changes.mapPos(pos), collapsed);
		}
		for (const effect of tr.effects) {
			if (!effect.is(setCalloutFold)) continue;
			if (next === value) next = new Map(value);
			next.set(effect.value.pos, effect.value.collapsed);
		}
		return next;
	}
});

function obsidianCallouts(state: EditorState): Callout[] {
	const doc = state.doc;
	const out: Callout[] = [];
	syntaxTree(state).iterate({
		enter(ref) {
			if (ref.name !== 'Blockquote') return;
			const header = doc.lineAt(ref.from);
			const match = OBSIDIAN.exec(header.text);
			if (!match) return;
			const open = header.text.indexOf('[!');
			const close = header.text.indexOf(']', open);
			const fold = match[2] === '+' ? '+' : match[2] === '-' ? '-' : null;
			let end = close + 1 + (fold ? 1 : 0);
			while (header.text[end] === ' ' || header.text[end] === '\t') end++;
			const last = doc.lineAt(ref.to);
			out.push({
				kind: calloutKind(match[1]),
				header,
				last,
				fold,
				markerFrom: header.from + open,
				markerTo: header.to,
				title: header.text.slice(end).trim(),
				bodyTo: last.to,
				close: null,
				dropFrom: null,
				dropTo: null
			});
		}
	});
	return out;
}

function colonCallouts(state: EditorState): Callout[] {
	const doc = state.doc;
	const nodes: SyntaxNode[] = [];
	syntaxTree(state).iterate({
		enter(ref) {
			if (ref.name === COLON_CALLOUT) nodes.push(ref.node);
		}
	});
	const out: Callout[] = [];
	for (let i = 0; i < nodes.length; i++) {
		const node = nodes[i];
		const header = doc.lineAt(node.from);
		const match = COLON_OPEN.exec(header.text);
		if (!match) continue;
		const close = nodes.find((other, j) => j > i && COLON_CLOSE.test(doc.lineAt(other.from).text));
		if (!close) continue;
		const closeLine = doc.lineAt(close.from);
		const before = doc.line(Math.max(1, closeLine.number - 1));
		let end = node.to - header.from;
		while (header.text[end] === ' ' || header.text[end] === '\t') end++;
		out.push({
			kind: calloutKind(match[1]),
			header,
			last: before,
			fold: null,
			markerFrom: node.from,
			markerTo: header.to,
			title: header.text.slice(end).trim(),
			bodyTo: before.to,
			close: closeLine,
			dropFrom: Math.max(0, closeLine.from - 1),
			dropTo: closeLine.to
		});
	}
	return out;
}

export function calloutsOf(state: EditorState): Callout[] {
	return [...obsidianCallouts(state), ...colonCallouts(state)];
}

class CalloutHeadWidget extends WidgetType {
	readonly kind: CalloutKind;
	readonly label: string;
	readonly fold: boolean;
	readonly collapsed: boolean;
	readonly pos: number;

	constructor(kind: CalloutKind, label: string, fold: boolean, collapsed: boolean, pos: number) {
		super();
		this.kind = kind;
		this.label = label;
		this.fold = fold;
		this.collapsed = collapsed;
		this.pos = pos;
	}

	eq(other: CalloutHeadWidget): boolean {
		return (
			other.kind === this.kind &&
			other.label === this.label &&
			other.fold === this.fold &&
			other.collapsed === this.collapsed &&
			other.pos === this.pos
		);
	}

	ignoreEvent(): boolean {
		return false;
	}

	toDOM(view: EditorView): HTMLElement {
		const head = document.createElement('span');
		head.className = 'cm-lp-callout-head';
		head.contentEditable = 'false';
		if (this.fold) {
			const button = document.createElement('button');
			button.type = 'button';
			button.className = 'cm-lp-callout-fold';
			button.setAttribute('aria-label', this.collapsed ? m.callout_expand() : m.callout_collapse());
			button.appendChild(chevronElement(this.collapsed));
			button.addEventListener('mousedown', (event) => {
				event.preventDefault();
				event.stopPropagation();
			});
			button.addEventListener('click', (event) => {
				event.preventDefault();
				event.stopPropagation();
				view.dispatch({
					effects: [
						setCalloutFold.of({ pos: this.pos, collapsed: !this.collapsed }),
						refreshDecorations.of(null)
					]
				});
			});
			head.appendChild(button);
		}
		head.appendChild(calloutIcon(this.kind));
		if (this.label) {
			const text = document.createElement('span');
			text.className = 'cm-lp-callout-label';
			text.textContent = this.label;
			head.appendChild(text);
		}
		return head;
	}
}

function buildCallouts(state: EditorState): DecorationSet {
	const touchedSet = touchedLines(state);
	const fold = state.field(calloutFold);
	const doc = state.doc;
	const ranges: Range<Decoration>[] = [];

	for (const callout of calloutsOf(state)) {
		const collapsed = fold.get(callout.header.from) ?? callout.fold === '-';
		const inside = touched(
			state,
			touchedSet,
			callout.header.from,
			callout.dropTo ?? callout.last.to
		);
		const lastNumber = collapsed
			? callout.header.number
			: inside && callout.close
				? callout.close.number
				: callout.last.number;
		for (let number = callout.header.number; number <= lastNumber; number++) {
			const at = doc.line(number);
			const classes = ['cm-lp-callout', `cm-lp-callout-${callout.kind}`];
			if (number === callout.header.number) classes.push('cm-lp-callout-first');
			if (number === lastNumber) classes.push('cm-lp-callout-last');
			ranges.push(line(at.from, classes.join(' ')));
		}
		if (inside) {
			ranges.push(mark(callout.markerFrom, callout.markerTo, 'cm-lp-dim'));
			continue;
		}
		ranges.push(
			Decoration.replace({
				widget: new CalloutHeadWidget(
					callout.kind,
					callout.title || CALLOUT_LABELS[callout.kind](),
					callout.fold !== null,
					collapsed,
					callout.header.from
				)
			}).range(callout.markerFrom, callout.markerTo)
		);
	}

	return Decoration.set(ranges, true);
}

function hiddenBodies(state: EditorState): DecorationSet {
	const touchedSet = touchedLines(state);
	const fold = state.field(calloutFold);
	const ranges: Range<Decoration>[] = [];
	for (const callout of calloutsOf(state)) {
		if (touched(state, touchedSet, callout.header.from, callout.dropTo ?? callout.last.to))
			continue;
		if (fold.get(callout.header.from) ?? callout.fold === '-')
			ranges.push(hide(callout.header.to, callout.bodyTo));
		if (callout.dropFrom !== null)
			ranges.push(hide(callout.dropFrom, callout.dropTo ?? callout.bodyTo));
	}
	return Decoration.set(ranges, true);
}

// Only a state field may replace a line break, and a folded body spans lines.
const calloutHides = StateField.define<DecorationSet>({
	create: (state) => hiddenBodies(state),
	update(value, tr) {
		if (
			tr.docChanged ||
			!tr.newSelection.eq(tr.startState.selection) ||
			tr.effects.some((e) => e.is(refreshDecorations))
		)
			return hiddenBodies(tr.state);
		return value.map(tr.changes);
	},
	provide: (field) => EditorView.decorations.from(field)
});

class LiveCallouts {
	decorations: DecorationSet;

	constructor(view: EditorView) {
		this.decorations = buildCallouts(view.state);
	}

	update(update: ViewUpdate) {
		if (
			!update.docChanged &&
			!update.selectionSet &&
			!update.viewportChanged &&
			!update.transactions.some((tr) => tr.effects.some((e) => e.is(refreshDecorations)))
		) {
			return;
		}
		this.decorations = buildCallouts(update.state);
	}
}

export const liveCallouts: readonly Extension[] = [
	calloutFold,
	calloutHides,
	ViewPlugin.fromClass(LiveCallouts, { decorations: (plugin) => plugin.decorations })
];
