import type { EditorState, Extension, Range } from '@codemirror/state';
import { Prec, StateField } from '@codemirror/state';
import {
	Decoration,
	EditorView,
	ViewPlugin,
	keymap,
	type DecorationSet,
	type ViewUpdate
} from '@codemirror/view';
import { contextOf, staticPreview } from './context';
import { collapsedLines, frontmatterEnd, hide, refreshDecorations } from './decorate';
import { parseFrontmatter } from './frontmatter';
import { PropertiesWidget } from './properties-panel';
import { focusPanel, propertiesUi, propertyKind, setPropertiesUi } from './property-edits';
import './properties.css';

// Only a state field may replace line breaks, and the frontmatter spans lines. Embeds drop it, as in Obsidian.
function propertiesDecorations(state: EditorState): DecorationSet {
	const embedded = state.facet(staticPreview);
	const ui = state.field(propertiesUi);
	const frontmatter = parseFrontmatter(state);
	if (!frontmatter) {
		if (embedded || !ui.adding || frontmatterEnd(state)) return Decoration.none;
		const widget = new PropertiesWidget([], ui.collapsed, true);
		return Decoration.set(Decoration.widget({ block: true, widget, side: -1 }).range(0));
	}
	let end = frontmatter.end;
	while (embedded && end < state.doc.length && !state.doc.lineAt(end + 1).text.trim())
		end = state.doc.lineAt(end + 1).to;
	const ranges: Range<Decoration>[] = [hide(0, end), ...collapsedLines(state, 0, end)];
	if (!embedded) {
		const entries = frontmatter.properties.map((property) => ({
			property,
			kind: propertyKind(state, property)
		}));
		const widget = new PropertiesWidget(entries, ui.collapsed, ui.adding);
		ranges.push(Decoration.widget({ block: true, widget, side: 1 }).range(end));
	}
	return Decoration.set(ranges, true);
}

const propertiesField = StateField.define<DecorationSet>({
	create: (state) => propertiesDecorations(state),
	update(value, tr) {
		if (
			tr.docChanged ||
			tr.effects.some((effect) => effect.is(setPropertiesUi) || effect.is(refreshDecorations))
		)
			return propertiesDecorations(tr.state);
		return value;
	},
	provide: (field) => EditorView.decorations.from(field)
});

function panelEnd(state: EditorState): number {
	if (state.facet(staticPreview) || !state.field(propertiesField, false)?.size) return 0;
	return frontmatterEnd(state);
}

function propertyCount(state: EditorState): number {
	return parseFrontmatter(state)?.properties.length ?? 0;
}
function leaveUpward(view: EditorView): boolean {
	if (view.state.facet(staticPreview)) return false;
	const range = view.state.selection.main;
	if (!range.empty || view.state.selection.ranges.length > 1) return false;
	const end = panelEnd(view.state);
	const doc = view.state.doc;
	const first = end ? doc.lineAt(end).number + 1 : 1;
	if (first > doc.lines || doc.lineAt(range.head).number !== first) return false;
	const moved = view.moveVertically(range, false);
	if (moved.head !== range.head && moved.head > end) return false;
	if (end && propertyCount(view.state)) focusPanel(view, { part: 'add' });
	else contextOf(view.state).focusTitle();
	return true;
}

function addFileProperty(view: EditorView): boolean {
	if (view.state.facet(staticPreview)) return false;
	view.dispatch({ effects: setPropertiesUi.of({ adding: true, collapsed: false }) });
	focusPanel(view, { part: 'new' });
	return true;
}

function guardFence(view: EditorView): boolean {
	const range = view.state.selection.main;
	const end = panelEnd(view.state);
	return !!end && range.empty && range.head === end + 1;
}

const caretBelowPanel = ViewPlugin.fromClass(
	class {
		constructor(readonly view: EditorView) {
			this.check();
		}

		update(update: ViewUpdate): void {
			if (
				update.selectionSet ||
				update.docChanged ||
				update.transactions.some((tr) => tr.reconfigured)
			)
				this.check();
		}

		check(): void {
			const { view } = this;
			const caret = view.state.selection.main;
			const end = panelEnd(view.state);
			if (!end || !caret.empty || caret.head > end) return;
			queueMicrotask(() => {
				const now = panelEnd(view.state);
				const main = view.state.selection.main;
				if (now && main.empty && main.head <= now)
					view.dispatch({ selection: { anchor: Math.min(view.state.doc.length, now + 1) } });
			});
		}
	}
);

export const liveProperties: Extension = [
	propertiesUi,
	propertiesField,
	caretBelowPanel,
	Prec.high(
		keymap.of([
			{ key: 'Mod-;', run: addFileProperty },
			{ key: 'ArrowUp', run: leaveUpward },
			{ key: 'Backspace', run: guardFence }
		])
	)
];
