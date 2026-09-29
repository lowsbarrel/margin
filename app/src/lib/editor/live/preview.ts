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
import { webHref } from '$lib/utils/web-link';
import { assetsOf } from './assets';
import { codeBlockDecorations } from './code-block';
import { CodeSpanCache } from './code-highlight';
import { contextOf, staticPreview } from './context';
import {
	eachLine,
	frontmatterEnd,
	hide,
	line,
	mark,
	refreshDecorations,
	touched,
	treeChanged
} from './decorate';
import { near, revealMoved, revealPoints, touchedLines } from './reveal';
import { mermaidOff } from './mermaid';
import { ListMarkWidget, SubpathWidget, TaskWidget } from './widgets';
import {
	COMMENT,
	COMMENT_BLOCK,
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

const LINK_OWNERS: Record<string, true> = {
	Link: true,
	Image: true,
	Autolink: true,
	LinkReference: true
};

function hideSpaceAfter(state: EditorState, to: number): number {
	return state.doc.sliceString(to, to + 1) === ' ' ? 1 : 0;
}

const EXTERNAL_URL = /^(https?:|mailto:)/;

const FRONTMATTER_ENTRY = /^(\s*)([^:\s][^:]*):(.*)$/;
const QUOTE_MARKS = /^(?:[ \t]*>)+/;

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

function quoteLines(
	state: EditorState,
	from: number,
	to: number,
	touchedSet: ReadonlySet<number>,
	ranges: Range<Decoration>[]
) {
	let depth = 1;
	eachLine(state, from, to, (at) => {
		const marks = QUOTE_MARKS.exec(at.text);
		if (marks) {
			depth = marks[0].split('>').length - 1;
			const end = at.from + marks[0].length;
			const shown = touchedSet.has(at.number) ? 'cm-lp-dim' : 'cm-lp-quote-hidden';
			const to = Math.min(end + hideSpaceAfter(state, end), at.to);
			ranges.push(mark(at.from, to, `cm-lp-quote-mark ${shown}`));
		}
		const attributes = depth > 1 ? { style: `--quote-depth: ${depth}` } : undefined;
		ranges.push(Decoration.line({ class: 'cm-lp-quote', attributes }).range(at.from));
	});
}

function commentDecorations(
	state: EditorState,
	from: number,
	to: number,
	block: boolean,
	ranges: Range<Decoration>[]
): void {
	if (!state.facet(staticPreview)) {
		ranges.push(mark(from, to, 'cm-lp-comment'));
		return;
	}
	eachLine(state, from, to, (at) => {
		const start = Math.max(at.from, from);
		const end = Math.min(at.to, to);
		if (end > start) ranges.push(hide(start, end));
		if (block) ranges.push(line(at.from, 'cm-lp-block-line'));
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

function subpathSeparators(target: string, from: number, ranges: Range<Decoration>[]): void {
	for (let at = target.indexOf('#'); at >= 0; at = target.indexOf('#', at + 1)) {
		if (at === 0) ranges.push(hide(from, from + 1));
		else
			ranges.push(
				Decoration.replace({ widget: new SubpathWidget() }).range(from + at, from + at + 1)
			);
	}
}

function build(view: EditorView, spans: CodeSpanCache): DecorationSet {
	const { state } = view;
	const { from: viewFrom, to: viewTo } = view.viewport;
	const ctx = contextOf(state);
	const touchedSet = touchedLines(state);
	const points = revealPoints(state);
	const frontmatter = frontmatterEnd(state);
	const doc = state.doc;
	const ranges: Range<Decoration>[] = [];

	const editingFrontmatter =
		frontmatter > 0 && !state.facet(staticPreview) && touched(state, touchedSet, 0, frontmatter);
	if (editingFrontmatter) frontmatterDecorations(state, frontmatter, ranges);

	for (const asset of assetsOf(state, viewFrom, viewTo)) {
		if (near(points, asset.from, asset.to)) continue;
		ranges.push(hide(asset.from, asset.to));
		if (asset.ownLine) ranges.push(line(asset.line.from, 'cm-lp-asset-line'));
	}

	syntaxTree(state).iterate({
		from: viewFrom,
		to: viewTo,
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
				codeBlockDecorations(state, ctx, touchedSet, node, ranges, spans);
				return false;
			}
			if (name === 'Blockquote') {
				if (node.parent?.name !== 'Blockquote')
					quoteLines(state, ref.from, ref.to, touchedSet, ranges);
				return;
			}
			if (name === 'HorizontalRule') return;
			if (name === COMMENT || name === COMMENT_BLOCK) {
				commentDecorations(state, ref.from, ref.to, name === COMMENT_BLOCK, ranges);
				return false;
			}

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
				const title = node.getChild('LinkTitle');
				if (!near(points, ref.from, ref.to)) {
					if (title) ranges.push(hide(url.to, title.from));
					if (EXTERNAL_URL.test(href))
						ranges.push(Decoration.widget({ widget: new ExternalLinkWidget() }).range(ref.to));
				}
				return;
			}

			if (name === 'Escape') {
				if (!near(points, ref.from, ref.to)) ranges.push(hide(ref.from, ref.from + 1));
				return;
			}

			if (name === 'Image' || name === EMBED) {
				if (near(points, ref.from, ref.to)) {
					ranges.push(mark(ref.from, ref.to, 'cm-lp-image-source'));
				}
				return;
			}

			if (name === 'URL' && !LINK_OWNERS[node.parent?.name ?? '']) {
				const href = webHref(doc.sliceString(ref.from, ref.to));
				ranges.push(mark(ref.from, ref.to, 'cm-lp-link', { title: href }));
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
					if (name === WIKI_LINK_TARGET && markCount(parent) < 3) {
						if (parent && !near(points, parent.from, parent.to))
							subpathSeparators(doc.sliceString(ref.from, ref.to), ref.from, ranges);
						return;
					}
				} else if (name === EMBED_MARK) {
					if (parentName !== EMBED) return;
				} else if (linkPart) {
					const autolink = name === 'LinkMark' && parentName === 'Autolink';
					if (!autolink && (parentName !== 'Link' || !parent?.getChild('URL'))) return;
				}
				const unit = parent ?? node;
				if (near(points, unit.from, unit.to)) ranges.push(mark(ref.from, ref.to, 'cm-lp-dim'));
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

			if (name === 'ListMark') {
				const item = node.parent;
				const ordered = item?.parent?.name === 'OrderedList';
				const task = item?.getChild('Task');
				const to = Math.min(ref.to + hideSpaceAfter(state, ref.to), doc.lineAt(ref.from).to);
				if (near(points, ref.from, ref.to)) {
					const source = ordered ? 'cm-lp-number-source' : 'cm-lp-bullet-source';
					ranges.push(mark(ref.from, to, `cm-lp-mark ${source}`));
					return;
				}
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
				const lineAt = doc.lineAt(ref.from);
				const end = Math.min(marker.to + hideSpaceAfter(state, marker.to), lineAt.to);
				ranges.push(line(lineAt.from, checked ? 'cm-lp-task cm-lp-done' : 'cm-lp-task'));
				if (checked && end < lineAt.to) ranges.push(mark(end, lineAt.to, 'cm-lp-done-text'));
				if (near(points, marker.from, marker.to)) {
					ranges.push(mark(marker.from, marker.to, 'cm-lp-dim'));
					return;
				}
				ranges.push(
					Decoration.replace({ widget: new TaskWidget(checked) }).range(marker.from, end)
				);
				return;
			}
		}
	});

	spans.commit();
	return Decoration.set(ranges, true);
}

class LivePreview {
	decorations: DecorationSet;
	private readonly spans = new CodeSpanCache();

	constructor(view: EditorView) {
		this.decorations = build(view, this.spans);
	}

	update(update: ViewUpdate) {
		if (
			!update.docChanged &&
			!update.viewportChanged &&
			!update.transactions.some(revealMoved) &&
			!treeChanged(update) &&
			!update.transactions.some((tr) => tr.effects.some((e) => e.is(refreshDecorations)))
		) {
			return;
		}
		this.decorations = build(update.view, this.spans);
	}
}

export const livePreview = ViewPlugin.fromClass(LivePreview, {
	decorations: (plugin) => plugin.decorations
});
