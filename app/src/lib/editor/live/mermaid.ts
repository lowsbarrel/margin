import { syntaxTree } from '@codemirror/language';
import type { SyntaxNode } from '@lezer/common';
import type { EditorState, Extension, Line } from '@codemirror/state';
import { StateField } from '@codemirror/state';
import {
	Decoration,
	EditorView,
	ViewPlugin,
	WidgetType,
	type DecorationSet
} from '@codemirror/view';
import * as m from '$lib/paraglide/messages.js';
import { collapsedLines, hide, refreshDecorations, touched, touchedLines } from './decorate';

interface MermaidBlock {
	from: number;
	to: number;
	line: Line;
	code: string;
}

let renderSeq = 0;

type MermaidTheme = 'default' | 'dark';

function themeName(): MermaidTheme {
	return document.documentElement.getAttribute('data-theme') === 'light' ? 'default' : 'dark';
}

function codeFences(node: SyntaxNode): SyntaxNode[] {
	const fences: SyntaxNode[] = [];
	for (let child = node.firstChild; child; child = child.nextSibling) {
		if (child.name === 'CodeMark') fences.push(child);
	}
	return fences;
}

function mermaidCode(state: EditorState, node: SyntaxNode): string | null {
	const info = node.getChild('CodeInfo');
	if (!info) return null;
	const language = state.doc.sliceString(info.from, info.to).trim().split(/\s+/)[0] ?? '';
	if (language.toLowerCase() !== 'mermaid') return null;
	const doc = state.doc;
	const fences = codeFences(node);
	if (fences.length === 0) return doc.sliceString(node.from, node.to);
	const first = doc.lineAt(fences[0].from);
	const from = Math.min(first.to + 1, doc.length);
	const closing = fences.length > 1 ? doc.lineAt(fences[fences.length - 1].from).from : node.to;
	return doc.sliceString(from, Math.max(from, closing - 1));
}

export function mermaidBlocks(state: EditorState, touchedSet: Set<number>): MermaidBlock[] {
	const doc = state.doc;
	const out: MermaidBlock[] = [];
	syntaxTree(state).iterate({
		enter(ref) {
			if (ref.name !== 'FencedCode') return;
			const code = mermaidCode(state, ref.node);
			if (code === null) return;
			if (touched(state, touchedSet, ref.from, ref.to)) return;
			const at = doc.lineAt(ref.from);
			out.push({ from: at.from, to: ref.to, line: at, code });
		}
	});
	return out;
}

export function mermaidOff(state: EditorState, node: SyntaxNode, touchedSet: Set<number>): boolean {
	if (node.name !== 'FencedCode') return false;
	if (mermaidCode(state, node) === null) return false;
	return !touched(state, touchedSet, node.from, node.to);
}

async function paintMermaid(target: HTMLElement, code: string, theme: MermaidTheme): Promise<void> {
	if (!code.trim()) {
		const empty = document.createElement('span');
		empty.className = 'cm-lp-mermaid-empty';
		empty.textContent = m.editor_empty_diagram();
		target.appendChild(empty);
		return;
	}
	try {
		// Mermaid is browser-only: AGENTS.md requires a dynamic import so the app never boots blank.
		const mod = await import('mermaid');
		const mermaid = mod.default;
		mermaid.initialize({
			startOnLoad: false,
			theme,
			securityLevel: 'strict',
			fontFamily: 'inherit'
		});
		await mermaid.parse(code);
		const { svg } = await mermaid.render(`cm-lp-mmd-${++renderSeq}`, code);
		target.innerHTML = svg;
	} catch (err) {
		target.textContent = '';
		const failure = document.createElement('div');
		failure.className = 'cm-lp-mermaid-error';
		failure.textContent = err instanceof Error ? err.message : String(err);
		target.appendChild(failure);
	}
}

class MermaidWidget extends WidgetType {
	readonly code: string;
	readonly theme: MermaidTheme;

	constructor(code: string, theme: MermaidTheme) {
		super();
		this.code = code;
		this.theme = theme;
	}

	eq(other: MermaidWidget): boolean {
		return other.code === this.code && other.theme === this.theme;
	}

	ignoreEvent(): boolean {
		return false;
	}

	toDOM(): HTMLElement {
		const diagram = document.createElement('div');
		diagram.className = 'cm-lp-mermaid';
		diagram.contentEditable = 'false';
		void paintMermaid(diagram, this.code, this.theme);
		return diagram;
	}
}

// Only a state field may replace a line break, and a fenced diagram spans lines.
function diagramDecorations(state: EditorState, touchedSet: Set<number>): DecorationSet {
	const ranges = mermaidBlocks(state, touchedSet).flatMap((block) => [
		hide(block.from, block.to),
		...collapsedLines(state, block.from, block.to),
		Decoration.widget({
			block: true,
			widget: new MermaidWidget(block.code, themeName()),
			side: 1
		}).range(block.to)
	]);
	return Decoration.set(ranges, true);
}

const mermaidDiagrams = StateField.define<DecorationSet>({
	create: (state) => diagramDecorations(state, touchedLines(state)),
	update(value, tr) {
		if (
			tr.docChanged ||
			!tr.newSelection.eq(tr.startState.selection) ||
			tr.effects.some((e) => e.is(refreshDecorations))
		)
			return diagramDecorations(tr.state, touchedLines(tr.state));
		return value.map(tr.changes);
	},
	provide: (field) => EditorView.decorations.from(field)
});

class ThemeWatcher {
	private observer: MutationObserver;

	constructor(readonly view: EditorView) {
		this.observer = new MutationObserver(() => {
			if (view.dom.isConnected) view.dispatch({ effects: refreshDecorations.of(null) });
		});
		this.observer.observe(document.documentElement, {
			attributes: true,
			attributeFilter: ['data-theme']
		});
	}

	destroy() {
		this.observer.disconnect();
	}
}

export const liveMermaid: readonly Extension[] = [
	mermaidDiagrams,
	ViewPlugin.fromClass(ThemeWatcher)
];
