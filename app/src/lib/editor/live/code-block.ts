import type { SyntaxNode } from '@lezer/common';
import type { EditorState, Range } from '@codemirror/state';
import { Decoration } from '@codemirror/view';
import type { LiveContext } from './context';
import type { CodeSpanCache } from './code-highlight';
import { eachLine, hide, line, mark, touched } from './decorate';
import { CodeHeaderWidget } from './widgets';

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

// The opening fence line carries the block's header, so the block height is the same raw and rendered.
function edgeClasses(state: EditorState, node: SyntaxNode, at: { number: number }): string {
	const doc = state.doc;
	const first = doc.lineAt(node.from).number;
	const last = doc.lineAt(Math.max(node.from, Math.min(node.to - 1, doc.length))).number;
	if (at.number === first && at.number === last) return ' cm-lp-code-first cm-lp-code-last';
	if (at.number === first) return ' cm-lp-code-first';
	if (at.number === last) return ' cm-lp-code-last';
	return '';
}

export function codeBlockDecorations(
	state: EditorState,
	ctx: LiveContext,
	touchedSet: Set<number>,
	node: SyntaxNode,
	ranges: Range<Decoration>[],
	spans: CodeSpanCache
): void {
	const doc = state.doc;
	eachLine(state, node.from, node.to, (at) =>
		ranges.push(line(at.from, `cm-lp-code${edgeClasses(state, node, at)}`))
	);
	if (node.name === 'CodeBlock') return;

	const info = node.getChild('CodeInfo');
	const language = info ? doc.sliceString(info.from, info.to).trim().split(/\s+/)[0] : '';
	const fences = fencesOf(node);
	const editable = touched(state, touchedSet, node.from, node.to);
	const content = codeRange(state, node, fences);
	const opening = doc.lineAt(node.from).from;

	for (const fence of fences) {
		const at = doc.lineAt(fence.from);
		if (editable) ranges.push(mark(fence.from, fence.to, 'cm-lp-dim'));
		else if (at.from === opening) {
			ranges.push(hide(at.from, at.to));
			ranges.push(
				Decoration.widget({
					widget: new CodeHeaderWidget(language || '', doc.sliceString(content.from, content.to)),
					side: 1
				}).range(at.to)
			);
		} else ranges.push(hide(at.from, at.to));
	}

	if (!language || content.from >= content.to) return;
	for (const span of spans.spans(
		ctx.code,
		language,
		doc.sliceString(content.from, content.to),
		content.from
	)) {
		ranges.push(mark(span.from, span.to, span.className));
	}
}
