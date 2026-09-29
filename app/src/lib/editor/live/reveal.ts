import { syntaxTree } from '@codemirror/language';
import {
	StateEffect,
	StateField,
	type EditorSelection,
	type EditorState,
	type Extension,
	type Text,
	type Transaction
} from '@codemirror/state';
import type { SyntaxNode } from '@lezer/common';
import { ViewPlugin, type EditorView } from '@codemirror/view';
import { staticPreview } from './context';

export const freezeReveal = StateEffect.define<readonly number[] | null>();

const revealFreeze = StateField.define<readonly number[] | null>({
	create: () => null,
	update(value, tr) {
		for (const effect of tr.effects) if (effect.is(freezeReveal)) return effect.value;
		return value && tr.docChanged ? value.map((pos) => tr.changes.mapPos(pos)) : value;
	}
});

function carets(selection: EditorSelection): readonly number[] {
	return selection.ranges.filter((range) => range.empty).map((range) => range.head);
}

// Only a caret reveals markup: a range selection keeps the rendered text so selecting never reflows it.
export function revealPoints(state: EditorState): readonly number[] {
	if (state.facet(staticPreview)) return [];
	return state.field(revealFreeze, false) ?? carets(state.selection);
}

export function touchedLines(state: EditorState): ReadonlySet<number> {
	return new Set(revealPoints(state).map((pos) => state.doc.lineAt(pos).number));
}

export function near(points: readonly number[], from: number, to: number): boolean {
	return points.some((pos) => pos >= from && pos <= to);
}

function lineKey(doc: Text, points: readonly number[]): string {
	return points.map((pos) => doc.lineAt(pos).number).join(' ');
}

function elementKey(state: EditorState): string {
	const tree = syntaxTree(state);
	const points = revealPoints(state);
	const parts = [lineKey(state.doc, points)];
	for (const pos of points) {
		for (const side of [-1, 1] as const) {
			for (let node: SyntaxNode | null = tree.resolveInner(pos, side); node; node = node.parent)
				parts.push(`${node.from}-${node.to}`);
		}
	}
	return parts.join(' ');
}

function mayMove(tr: Transaction): boolean {
	const start = tr.startState;
	if (start.facet(staticPreview)) return false;
	if (tr.effects.some((effect) => effect.is(freezeReveal))) return true;
	return !start.field(revealFreeze, false) && !start.selection.eq(tr.newSelection);
}

export function revealMoved(tr: Transaction): boolean {
	return mayMove(tr) && elementKey(tr.startState) !== elementKey(tr.state);
}

export function revealLinesMoved(tr: Transaction): boolean {
	if (!mayMove(tr)) return false;
	return (
		lineKey(tr.startState.doc, revealPoints(tr.startState)) !==
		lineKey(tr.newDoc, revealPoints(tr.state))
	);
}

class RevealHold {
	readonly view: EditorView;
	private controller: AbortController | null = null;

	constructor(view: EditorView) {
		this.view = view;
	}

	hold(): void {
		if (this.controller) return;
		this.controller = new AbortController();
		const { signal } = this.controller;
		const { state } = this.view;
		if (!state.field(revealFreeze, false))
			this.view.dispatch({ effects: freezeReveal.of(revealPoints(state)) });
		const release = () => this.release();
		window.addEventListener('mouseup', release, { signal });
		window.addEventListener('blur', release, { signal });
	}

	release(): void {
		if (!this.controller) return;
		this.controller.abort();
		this.controller = null;
		this.view.dispatch({ effects: freezeReveal.of(null) });
	}

	destroy(): void {
		this.controller?.abort();
		this.controller = null;
	}
}

const revealHold = ViewPlugin.fromClass(RevealHold, {
	eventHandlers: {
		mousedown(event) {
			if (event.button === 0 && !this.view.state.facet(staticPreview)) this.hold();
			return false;
		}
	}
});

export const liveReveal: Extension = [revealFreeze, revealHold];
