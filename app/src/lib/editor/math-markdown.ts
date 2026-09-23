import type { Node as PMNode } from '@tiptap/pm/model';

export interface MarkdownSerializerState {
	write(content: string): void;
	closeBlock(node: PMNode): void;
}

interface MdToken {
	content: string;
	attrPush(attr: [string, string]): void;
	attrGet(name: string): string | null;
	map: [number, number] | null;
}

interface MdBlockState {
	src: string;
	bMarks: number[];
	eMarks: number[];
	tShift: number[];
	line: number;
	push(type: string, tag: string, nesting: number): MdToken;
}

interface MdInlineState {
	src: string;
	pos: number;
	posMax: number;
	push(type: string, tag: string, nesting: number): MdToken;
}

type MdBlockRule = (
	state: MdBlockState,
	startLine: number,
	endLine: number,
	silent: boolean
) => boolean;

type MdInlineRule = (state: MdInlineState, silent: boolean) => boolean;

type MdRenderRule = (tokens: MdToken[], idx: number) => string;

export interface MarkdownIt {
	utils: { escapeHtml(str: string): string };
	block: { ruler: { before(beforeName: string, name: string, rule: MdBlockRule): void } };
	inline: { ruler: { push(name: string, rule: MdInlineRule): void } };
	renderer: { rules: Record<string, MdRenderRule> };
}

export function mathBlockMarkdownPlugin(md: MarkdownIt) {
	md.block.ruler.before('fence', 'math_block', function (state, startLine, endLine, silent) {
		const pos = state.bMarks[startLine] + state.tShift[startLine];
		const max = state.eMarks[startLine];
		const src = state.src;

		if (pos + 2 > max) return false;
		if (src.charCodeAt(pos) !== 0x24 || src.charCodeAt(pos + 1) !== 0x24) return false;
		const rest = src.slice(pos + 2, max).trim();
		if (rest) return false;

		if (silent) return true;

		let nextLine = startLine + 1;
		let found = false;
		for (; nextLine < endLine; nextLine++) {
			const npos = state.bMarks[nextLine] + state.tShift[nextLine];
			const nmax = state.eMarks[nextLine];
			const line = src.slice(npos, nmax).trim();
			if (line === '$$') {
				found = true;
				break;
			}
		}
		if (!found) return false;
		const lines: string[] = [];
		for (let i = startLine + 1; i < nextLine; i++) {
			lines.push(src.slice(state.bMarks[i] + state.tShift[i], state.eMarks[i]));
		}
		const mathText = lines.join('\n');

		const token = state.push('math_block', 'div', 0);
		token.attrPush(['data-type', 'mathBlock']);
		token.attrPush(['data-math-text', mathText]);
		token.content = mathText;
		token.map = [startLine, nextLine + 1];

		state.line = nextLine + 1;
		return true;
	});

	md.renderer.rules.math_block = function (tokens, idx) {
		const token = tokens[idx];
		const text = token.attrGet('data-math-text') || token.content || '';
		const escaped = md.utils.escapeHtml(text);
		return `<div data-type="mathBlock" data-math-text="${escaped}">${escaped}</div>`;
	};
}

export function mathInlineMarkdownPlugin(md: MarkdownIt) {
	md.inline.ruler.push('math_inline', function (state, silent) {
		const max = state.posMax;
		const src = state.src;
		const pos = state.pos;

		if (pos + 2 >= max) return false;
		if (src.charCodeAt(pos) !== 0x24) return false;
		if (src.charCodeAt(pos + 1) === 0x24) return false;

		let closePos = -1;
		for (let i = pos + 1; i < max; i++) {
			const code = src.charCodeAt(i);
			if (code === 0x5c) {
				i++;
				continue;
			}
			if (code === 0x24) {
				closePos = i;
				break;
			}
		}
		if (closePos === -1 || closePos >= max) return false;
		const mathText = src.slice(pos + 1, closePos);
		if (!mathText.trim()) return false;

		if (silent) return true;

		const token = state.push('math_inline', 'span', 0);
		token.attrPush(['data-type', 'mathInline']);
		token.attrPush(['data-math-text', mathText]);
		token.content = mathText;

		state.pos = closePos + 1;
		return true;
	});

	md.renderer.rules.math_inline = function (tokens, idx) {
		const token = tokens[idx];
		const text = token.attrGet('data-math-text') || token.content || '';
		const escaped = md.utils.escapeHtml(text);
		return `<span data-type="mathInline" data-math-text="${escaped}">${escaped}</span>`;
	};
}
