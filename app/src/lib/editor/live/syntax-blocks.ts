import type { BlockParser, MarkdownConfig } from '@lezer/markdown';
import { tags } from '@lezer/highlight';

export const BLOCK_MATH = 'BlockMath';
export const BLOCK_MATH_MARK = 'BlockMathMark';
export const COLON_CALLOUT = 'ColonCallout';
const COLON_CALLOUT_MARK = 'ColonCalloutMark';
export const FRONTMATTER = 'Frontmatter';
const FRONTMATTER_MARK = 'FrontmatterMark';

const FENCE = /^\$\$\s*$/;
const ONELINE = /^\$\$(.+?)\$\$\s*$/;
const COLONS = /^:::\s*([A-Za-z][\w-]*)?/;

const blockMath: BlockParser = {
	name: BLOCK_MATH,
	parse(cx, line) {
		const text = line.text.slice(line.pos);
		const from = cx.lineStart + line.pos;
		const inline = ONELINE.exec(text);
		if (inline) {
			const to = cx.lineStart + line.pos + inline[0].replace(/\s+$/, '').length;
			cx.nextLine();
			cx.addElement(
				cx.elt(BLOCK_MATH, from, to, [
					cx.elt(BLOCK_MATH_MARK, from, from + 2),
					cx.elt(BLOCK_MATH_MARK, to - 2, to)
				])
			);
			return true;
		}
		if (!FENCE.test(text)) return false;
		const marks = [cx.elt(BLOCK_MATH_MARK, from, from + 2)];
		while (cx.nextLine()) {
			if (FENCE.test(line.text.slice(line.pos))) {
				const at = cx.lineStart + line.pos;
				marks.push(cx.elt(BLOCK_MATH_MARK, at, at + 2));
				cx.nextLine();
				break;
			}
		}
		cx.addElement(cx.elt(BLOCK_MATH, from, cx.prevLineEnd(), marks));
		return true;
	},
	endLeaf(cx, line) {
		const text = line.text.slice(line.pos);
		return FENCE.test(text) || ONELINE.test(text);
	}
};

const colonCallout: BlockParser = {
	name: COLON_CALLOUT,
	parse(cx, line) {
		const match = COLONS.exec(line.text.slice(line.pos));
		if (!match) return false;
		const from = cx.lineStart + line.pos;
		const to = from + match[0].replace(/\s+$/, '').length;
		cx.nextLine();
		cx.addElement(cx.elt(COLON_CALLOUT, from, to, [cx.elt(COLON_CALLOUT_MARK, from, to)]));
		return true;
	},
	endLeaf(cx, line) {
		return COLONS.test(line.text.slice(line.pos));
	}
};

const frontmatter: BlockParser = {
	name: FRONTMATTER,
	before: 'HorizontalRule',
	parse(cx, line) {
		if (line.pos !== 0 || cx.lineStart !== 0 || !/^---\s*$/.test(line.text)) return false;
		const marks = [cx.elt(FRONTMATTER_MARK, 0, 3)];
		while (cx.nextLine()) {
			if (/^(---|\.\.\.)\s*$/.test(line.text)) {
				marks.push(
					cx.elt(FRONTMATTER_MARK, cx.lineStart + line.pos, cx.lineStart + line.text.length)
				);
				cx.nextLine();
				break;
			}
		}
		cx.addElement(cx.elt(FRONTMATTER, 0, cx.prevLineEnd(), marks));
		return true;
	}
};

export const BlockMath: MarkdownConfig = {
	defineNodes: [
		{ name: BLOCK_MATH, block: true },
		{ name: BLOCK_MATH_MARK, style: tags.processingInstruction }
	],
	parseBlock: [blockMath]
};

export const ColonCallout: MarkdownConfig = {
	defineNodes: [{ name: COLON_CALLOUT, block: true }, { name: COLON_CALLOUT_MARK }],
	parseBlock: [colonCallout]
};

export const Frontmatter: MarkdownConfig = {
	defineNodes: [{ name: FRONTMATTER, block: true }, { name: FRONTMATTER_MARK }],
	parseBlock: [frontmatter]
};
