import type { Element, InlineContext, InlineParser, MarkdownConfig } from '@lezer/markdown';
import { tags } from '@lezer/highlight';

export const WIKI_LINK = 'WikiLink';
export const WIKI_LINK_MARK = 'WikiLinkMark';
export const WIKI_LINK_TARGET = 'WikiLinkTarget';
export const EMBED = 'Embed';
export const EMBED_MARK = 'EmbedMark';
export const HIGHLIGHT = 'Highlight';
export const HIGHLIGHT_MARK = 'HighlightMark';
export const TAG = 'Tag';
export const INLINE_MATH = 'InlineMath';
export const INLINE_MATH_MARK = 'InlineMathMark';

const PUNCTUATION = /[!"#$%&'()*+,\-./:;<=>?@[\\\]^_`{|}~]/;

function lineEnd(cx: InlineContext, from: number): number {
	for (let pos = from; pos < cx.end; pos++) {
		if (cx.char(pos) === 10) return pos;
	}
	return cx.end;
}

function find(cx: InlineContext, from: number, char: number, to: number): number {
	for (let pos = from; pos < to; pos++) {
		if (cx.char(pos) === char) return pos;
	}
	return -1;
}

// Obsidian's tag alphabet: letters, digits, `_`, `-` and `/`, and the name needs a non-digit.
const TAG_NAME = /^[\p{L}\p{N}_/-]+/u;
const TAG_LETTER = /[\p{L}_]/u;

function scanTag(cx: InlineContext, from: number, to: number): number {
	const match = TAG_NAME.exec(cx.slice(from, to));
	if (!match) return from;
	let end = from + match[0].length;
	while (cx.slice(end - 1, end) === '/') end--;
	return TAG_LETTER.test(match[0]) ? end : from;
}

const wikiLink: InlineParser = {
	name: WIKI_LINK,
	before: 'Link',
	parse(cx, next, pos) {
		if (next !== 91 || cx.char(pos + 1) !== 91) return -1;
		const stop = lineEnd(cx, pos + 2);
		const end = find(cx, pos + 2, 93, stop);
		if (end < 0 || cx.char(end + 1) !== 93) return -1;
		const pipe = find(cx, pos + 2, 124, end);
		const children: Element[] = [
			cx.elt(WIKI_LINK_MARK, pos, pos + 2),
			cx.elt(WIKI_LINK_TARGET, pos + 2, pipe < 0 ? end : pipe)
		];
		if (pipe >= 0) children.push(cx.elt(WIKI_LINK_MARK, pipe, pipe + 1));
		children.push(cx.elt(WIKI_LINK_MARK, end, end + 2));
		return cx.addElement(cx.elt(WIKI_LINK, pos, end + 2, children));
	}
};

const embed: InlineParser = {
	name: EMBED,
	before: 'Link',
	parse(cx, next, pos) {
		if (next !== 33 || cx.char(pos + 1) !== 91 || cx.char(pos + 2) !== 91) return -1;
		if (/\w/.test(cx.slice(pos - 1, pos))) return -1;
		const stop = lineEnd(cx, pos + 3);
		const end = find(cx, pos + 3, 93, stop);
		if (end < 0 || cx.char(end + 1) !== 93) return -1;
		return cx.addElement(
			cx.elt(EMBED, pos, end + 2, [
				cx.elt(EMBED_MARK, pos, pos + 3),
				cx.elt(EMBED_MARK, end, end + 2)
			])
		);
	}
};

const HighlightDelim = { resolve: HIGHLIGHT, mark: HIGHLIGHT_MARK };

const highlight: MarkdownConfig = {
	defineNodes: [
		{ name: HIGHLIGHT, style: { [`${HIGHLIGHT}/...`]: tags.special(tags.content) } },
		{ name: HIGHLIGHT_MARK, style: tags.processingInstruction }
	],
	parseInline: [
		{
			name: HIGHLIGHT,
			after: 'Emphasis',
			parse(cx, next, pos) {
				if (next !== 61 || cx.char(pos + 1) !== 61 || cx.char(pos + 2) === 61) return -1;
				const before = cx.slice(pos - 1, pos);
				const after = cx.slice(pos + 2, pos + 3);
				const spaceBefore = /\s|^$/.test(before);
				const spaceAfter = /\s|^$/.test(after);
				const punctBefore = PUNCTUATION.test(before);
				const punctAfter = PUNCTUATION.test(after);
				return cx.addDelimiter(
					HighlightDelim,
					pos,
					pos + 2,
					!spaceAfter && (!punctAfter || spaceBefore || punctBefore),
					!spaceBefore && (!punctBefore || spaceAfter || punctAfter)
				);
			}
		}
	]
};

const tag: MarkdownConfig = {
	defineNodes: [{ name: TAG, style: tags.atom }],
	parseInline: [
		{
			name: TAG,
			before: 'Link',
			parse(cx, next, pos) {
				if (next !== 35) return -1;
				if (pos > 0 && !/[\s(>[\]]/.test(cx.slice(pos - 1, pos))) return -1;
				const end = scanTag(cx, pos + 1, lineEnd(cx, pos + 1));
				if (end === pos + 1) return -1;
				return cx.addElement(cx.elt(TAG, pos, end));
			}
		}
	]
};

const inlineMath: MarkdownConfig = {
	defineNodes: [
		{ name: INLINE_MATH, style: tags.special(tags.content) },
		{ name: INLINE_MATH_MARK, style: tags.processingInstruction }
	],
	parseInline: [
		{
			name: INLINE_MATH,
			before: 'Link',
			parse(cx, next, pos) {
				if (next !== 36 || cx.char(pos - 1) === 36 || cx.char(pos + 1) === 36) return -1;
				const first = cx.slice(pos + 1, pos + 2);
				if (!first || first === ' ' || first === '\\') return -1;
				const stop = lineEnd(cx, pos + 1);
				const end = find(cx, pos + 1, 36, stop);
				if (end < 0) return -1;
				const last = cx.slice(end - 1, end);
				if (!last || last === ' ' || last === '\\') return -1;
				if (/[0-9]/.test(cx.slice(end + 1, end + 2))) return -1;
				return cx.addElement(
					cx.elt(INLINE_MATH, pos, end + 1, [
						cx.elt(INLINE_MATH_MARK, pos, pos + 1),
						cx.elt(INLINE_MATH_MARK, end, end + 1)
					])
				);
			}
		}
	]
};

export const WikiLink: MarkdownConfig = {
	defineNodes: [
		{ name: WIKI_LINK, style: tags.link },
		{ name: WIKI_LINK_MARK, style: tags.processingInstruction },
		{ name: WIKI_LINK_TARGET }
	],
	parseInline: [wikiLink]
};

export const Embed: MarkdownConfig = {
	defineNodes: [
		{ name: EMBED, style: tags.link },
		{ name: EMBED_MARK, style: tags.processingInstruction }
	],
	parseInline: [embed]
};

export { highlight as Highlight, tag as Tag, inlineMath as InlineMath };
