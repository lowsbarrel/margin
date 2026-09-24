import type { SyntaxNode } from '@lezer/common';
import type { EditorState, Range } from '@codemirror/state';
import { Decoration } from '@codemirror/view';
import type { LiveContext } from './context';
import { highlightCode } from './code-highlight';
import { eachLine, hide, line, mark, touched } from './decorate';
import { CodeFenceWidget } from './widgets';

function fencesOf(node: SyntaxNode): SyntaxNode[] {
	const fences: SyntaxNode[] = [];
	for (let child = node.firstChild; child; child = child.nextSibling) {
		if (child.name === 'CodeMark') fences.push(child);
	}
	return fences;
}

function codeRange(state: EditorState, node: SyntaxNode, fences: SyntaxNode[]) {
	const doc = state.doc;
	const from = fences.length > 0 ? doc.lineAt(fences[0].from).to + 1 : node.from;
	const closing = fences.length > 1 ? fences[fences.length - 1] : null;
	const to = closing ? Math.max(from, doc.lineAt(closing.from).from - 1) : node.to;
	return { from, to: Math.min(to, doc.length) };
}

export function codeBlockDecorations(
	state: EditorState,
	ctx: LiveContext,
	touchedSet: Set<number>,
	node: SyntaxNode,
	ranges: Range<Decoration>[]
): void {
	const doc = state.doc;
	eachLine(state, node.from, node.to, (at) => ranges.push(line(at.from, 'cm-lp-code')));
	if (node.name === 'CodeBlock') return;

	const info = node.getChild('CodeInfo');
	const language = info ? doc.sliceString(info.from, info.to) : '';
	const fences = fencesOf(node);
	const editable = touched(state, touchedSet, node.from, node.to);
	for (const fence of fences) {
		const at = doc.lineAt(fence.from);
		if (editable) ranges.push(mark(fence.from, fence.to, 'cm-lp-dim'));
		else if (at.from === doc.lineAt(node.from).from) {
			ranges.push(
				Decoration.replace({
					widget: new CodeFenceWidget(language.trim().split(/\s+/)[0] ?? '')
				}).range(at.from, at.to)
			);
		} else ranges.push(hide(at.from, at.to));
	}

	if (!language || editable) return;
	const { from, to } = codeRange(state, node, fences);
	if (from >= to) return;
	for (const span of highlightCode(ctx.code, language, doc.sliceString(from, to), from)) {
		ranges.push(mark(span.from, span.to, span.className));
	}
}
