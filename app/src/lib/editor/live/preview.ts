import { syntaxTree } from '@codemirror/language';
import { type EditorState, type Range } from '@codemirror/state';
import type { SyntaxNode } from '@lezer/common';
import {
	Decoration,
	ViewPlugin,
	WidgetType,
	type DecorationSet,
	type EditorView,
	type ViewUpdate
} from '@codemirror/view';
import { assetsOf } from './assets';
import { codeBlockDecorations } from './code-block';
import { contextOf } from './context';
import {
	eachLine,
	frontmatterEnd,
	hide,
	line,
	mark,
	refreshDecorations,
	touched,
	touchedLines
} from './decorate';
import { mermaidOff } from './mermaid';
import { ListMarkWidget, TaskWidget } from './widgets';
import {
	EMBED,
	EMBED_MARK,
	HIGHLIGHT,
	HIGHLIGHT_MARK,
	TAG,
	WIKI_LINK,
	WIKI_LINK_MARK,
	WIKI_LINK_TARGET
} from './syntax';

const HEADINGS: Record<string, string> = {
	ATXHeading1: 'cm-lp-h1',
	ATXHeading2: 'cm-lp-h2',
	ATXHeading3: 'cm-lp-h3',
	ATXHeading4: 'cm-lp-h4',
	ATXHeading5: 'cm-lp-h5',
	ATXHeading6: 'cm-lp-h6',
	SetextHeading1: 'cm-lp-h1',
	SetextHeading2: 'cm-lp-h2'
};

const INLINE_CLASSES: Record<string, string> = {
	Emphasis: 'cm-lp-em',
	StrongEmphasis: 'cm-lp-strong',
	Strikethrough: 'cm-lp-strike',
	[HIGHLIGHT]: 'cm-lp-highlight',
	InlineCode: 'cm-lp-code-inline',
	Autolink: 'cm-lp-link',
	[WIKI_LINK]: 'cm-lp-wikilink',
	[TAG]: 'cm-lp-tag',
	HTMLTag: 'cm-lp-html',
	HTMLBlock: 'cm-lp-html'
};

const HIDDEN_MARKS: Record<string, true> = {
	EmphasisMark: true,
	StrikethroughMark: true,
	[HIGHLIGHT_MARK]: true,
	[WIKI_LINK_MARK]: true,
	[EMBED_MARK]: true
};

function hideSpaceAfter(state: EditorState, to: number): number {
	return state.doc.sliceString(to, to + 1) === ' ' ? 1 : 0;
}

const EXTERNAL_URL = /^(https?:|mailto:)/;

const FRONTMATTER_ENTRY = /^(\s*)([^:\s][^:]*):(.*)$/;

function frontmatterDecorations(
	state: EditorState,
	end: number,
	ranges: Range<Decoration>[]
): void {
	const doc = state.doc;
	const last = doc.lineAt(Math.max(0, end - 1)).number;
	eachLine(state, 0, end, (at) => {
		const first = at.number === 1 ? ' cm-lp-fm-first' : '';
		const closing = at.number === last ? ' cm-lp-fm-last' : '';
		ranges.push(line(at.from, `cm-lp-frontmatter${first}${closing}`));
		if (at.number === 1 || at.number === last) return;
		const entry = FRONTMATTER_ENTRY.exec(at.text);
		if (!entry) return;
		const key = at.from + entry[1].length;
		ranges.push(mark(key, key + entry[2].length, 'cm-lp-fm-key'));
		if (entry[3]) ranges.push(mark(key + entry[2].length + 1, at.to, 'cm-lp-fm-value'));
	});
}

class ExternalLinkWidget extends WidgetType {
	eq(): boolean {
		return true;
	}

	toDOM(): HTMLElement {
		const span = document.createElement('span');
		span.className = 'cm-lp-external';
		const svg = document.createElementNS('http://www.w3.org/2000/svg', 'svg');
		svg.setAttribute('viewBox', '0 0 16 16');
		svg.setAttribute('aria-hidden', 'true');
		for (const d of ['M6.5 3h6.5v6.5', 'M13 3 4.5 11.5']) {
			const path = document.createElementNS('http://www.w3.org/2000/svg', 'path');
			path.setAttribute('d', d);
			svg.append(path);
		}
		span.append(svg);
		return span;
	}
}

// Three marks means the wiki link carries a `|alias`, so the target is replaced by the alias.
function markCount(node: SyntaxNode | null): number {
	let count = 0;
	for (let child = node?.firstChild; child; child = child.nextSibling) {
		if (child.name === WIKI_LINK_MARK) count++;
	}
	return count;
}

function build(state: EditorState): DecorationSet {
	const ctx = contextOf(state);
	const touchedSet = touchedLines(state);
	const frontmatter = frontmatterEnd(state);
	const doc = state.doc;
	const ranges: Range<Decoration>[] = [];

	if (frontmatter > 0) frontmatterDecorations(state, frontmatter, ranges);

	for (const asset of assetsOf(state)) {
		if (touched(state, touchedSet, asset.from, asset.to)) continue;
		ranges.push(hide(asset.from, asset.to));
		if (asset.ownLine) ranges.push(line(asset.line.from, 'cm-lp-asset-line'));
	}

	syntaxTree(state).iterate({
		enter(ref) {
			if (ref.from < frontmatter) return;
			const name = ref.name;
			const node = ref.node;

			const heading = HEADINGS[name];
			if (heading) {
				eachLine(state, ref.from, ref.to, (at) => ranges.push(line(at.from, heading)));
				return;
			}
			if (name === 'FencedCode' || name === 'CodeBlock') {
				if (mermaidOff(state, node, touchedSet)) return false;
				codeBlockDecorations(state, ctx, touchedSet, node, ranges);
				return false;
			}
			if (name === 'Blockquote') {
				eachLine(state, ref.from, ref.to, (at) => ranges.push(line(at.from, 'cm-lp-quote')));
				return;
			}
			if (name === 'HorizontalRule') return;

			const inline = INLINE_CLASSES[name];
			if (inline) {
				ranges.push(mark(ref.from, ref.to, inline));
				return;
			}

			// A link without a URL is a footnote or a reference label; leave its brackets in place.
			if (name === 'Link') {
				const url = node.getChild('URL');
				if (!url) return;
				const href = doc.sliceString(url.from, url.to);
				ranges.push(mark(ref.from, ref.to, 'cm-lp-link', { title: href }));
				if (EXTERNAL_URL.test(href) && !touched(state, touchedSet, ref.from, ref.to)) {
					ranges.push(Decoration.widget({ widget: new ExternalLinkWidget() }).range(ref.to));
				}
				return;
			}

			if (name === 'Image' || name === EMBED) {
				if (touched(state, touchedSet, ref.from, ref.to)) {
					ranges.push(mark(ref.from, ref.to, 'cm-lp-image-source'));
				}
				return;
			}

			if (
				HIDDEN_MARKS[name] ||
				name === WIKI_LINK_TARGET ||
				name === 'CodeMark' ||
				name === 'LinkMark' ||
				name === 'URL' ||
				name === 'LinkTitle'
			) {
				const parent = node.parent;
				const parentName = parent?.name ?? '';
				const linkPart = name === 'LinkMark' || name === 'URL' || name === 'LinkTitle';
				if (name === 'CodeMark') {
					if (parentName !== 'InlineCode') return;
				} else if (name === WIKI_LINK_MARK || name === WIKI_LINK_TARGET) {
					if (parentName !== WIKI_LINK) return;
					if (name === WIKI_LINK_TARGET && markCount(parent) < 3) return;
				} else if (name === EMBED_MARK) {
					if (parentName !== EMBED) return;
				} else if (linkPart) {
					if (parentName !== 'Link' || !parent?.getChild('URL')) return;
				}
				if (touched(state, touchedSet, ref.from, ref.to))
					ranges.push(mark(ref.from, ref.to, 'cm-lp-dim'));
				else ranges.push(hide(ref.from, ref.to));
				return;
			}

			if (name === 'HeaderMark') {
				const parent = node.parent?.name ?? '';
				const at = doc.lineAt(ref.from);
				if (touched(state, touchedSet, ref.from, ref.to)) {
					ranges.push(mark(ref.from, ref.to, 'cm-lp-dim'));
					return;
				}
				if (parent.startsWith('ATXHeading')) {
					ranges.push(hide(ref.from, Math.min(ref.to + hideSpaceAfter(state, ref.to), at.to)));
				} else if (parent.startsWith('SetextHeading')) {
					ranges.push(hide(ref.from, ref.to));
					ranges.push(line(at.from, 'cm-lp-collapsed'));
				}
				return;
			}

			if (name === 'QuoteMark') {
				const to = Math.min(ref.to + hideSpaceAfter(state, ref.to), doc.lineAt(ref.from).to);
				if (touched(state, touchedSet, ref.from, ref.to))
					ranges.push(mark(ref.from, ref.to, 'cm-lp-dim'));
				else ranges.push(hide(ref.from, to));
				return;
			}

			if (name === 'ListMark') {
				const item = node.parent;
				const ordered = item?.parent?.name === 'OrderedList';
				const task = item?.getChild('Task');
				if (touched(state, touchedSet, ref.from, ref.to)) {
					ranges.push(mark(ref.from, ref.to, 'cm-lp-mark'));
					return;
				}
				const to = Math.min(ref.to + hideSpaceAfter(state, ref.to), doc.lineAt(ref.from).to);
				ranges.push(hide(ref.from, to));
				const text = ordered ? doc.sliceString(ref.from, ref.to) : '•';
				if (ordered || !task) {
					ranges.push(
						Decoration.replace({ widget: new ListMarkWidget(text, ordered) }).range(
							ref.from,
							ref.to
						)
					);
				}
				return;
			}

			if (name === 'Task') {
				const marker = node.getChild('TaskMarker');
				if (!marker) return;
				const checked = /[xX]/.test(doc.sliceString(marker.from, marker.to));
				ranges.push(
					line(doc.lineAt(ref.from).from, checked ? 'cm-lp-task cm-lp-done' : 'cm-lp-task')
				);
				if (touched(state, touchedSet, marker.from, marker.to)) {
					ranges.push(mark(marker.from, marker.to, 'cm-lp-dim'));
					return;
				}
				const end = Math.min(
					marker.to + hideSpaceAfter(state, marker.to),
					doc.lineAt(marker.from).to
				);
				ranges.push(
					Decoration.replace({ widget: new TaskWidget(checked) }).range(marker.from, end)
				);
				return;
			}
		}
	});

	return Decoration.set(ranges, true);
}

class LivePreview {
	decorations: DecorationSet;

	constructor(view: EditorView) {
		this.decorations = build(view.state);
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
		this.decorations = build(update.state);
	}
}

export const livePreview = ViewPlugin.fromClass(LivePreview, {
	decorations: (plugin) => plugin.decorations
});
