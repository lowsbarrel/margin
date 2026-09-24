import MarkdownIt, {
	type StateBlock,
	type StateCore,
	type StateInline,
	type Token
} from 'markdown-it';
import { common, createLowlight } from 'lowlight';
import { buildLocalfileUrl } from '$lib/editor/image-url';
import { imageSource, resolveEmbed, type ResolveSources } from '$lib/editor/live/resolve';

interface HastNode {
	type: string;
	value?: string;
	properties?: { className?: string[] | string };
	children?: HastNode[];
}

export interface KatexLike {
	renderToString(latex: string, options?: Record<string, unknown>): string;
}

export interface PdfRenderOptions {
	host: ResolveSources;
	katex: KatexLike;
}

const pdfLowlight = createLowlight(common);
const CALLOUT_TYPES: Record<string, true> = {
	note: true,
	info: true,
	success: true,
	warning: true,
	danger: true,
	tip: true
};
const MARKER = /^\[!([A-Za-z]+)\][ \t]*/;
const COLON_CALLOUT = /^:::+\s*([A-Za-z]+)\s*$/;
const COLON_END = /^:::+\s*$/;

export function escapeHtml(value: string): string {
	return value
		.replace(/&/g, '&amp;')
		.replace(/</g, '&lt;')
		.replace(/>/g, '&gt;')
		.replace(/"/g, '&quot;');
}

function hastToHtml(nodes: readonly HastNode[]): string {
	let out = '';
	for (const node of nodes) {
		if (node.type === 'text') {
			out += escapeHtml(node.value ?? '');
		} else if (node.type === 'element') {
			const cls = node.properties?.className;
			const className = Array.isArray(cls) ? cls.join(' ') : (cls ?? '');
			out += `<span class="${className}">${hastToHtml(node.children ?? [])}</span>`;
		}
	}
	return out;
}

function highlightCode(code: string, lang: string): string {
	if (!code.trim()) return '';
	try {
		const tree =
			lang && pdfLowlight.registered(lang)
				? pdfLowlight.highlight(lang, code)
				: pdfLowlight.highlightAuto(code);
		return hastToHtml(tree.children as unknown as HastNode[]);
	} catch {
		return '';
	}
}

function calloutType(name: string): string {
	const type = name.toLowerCase();
	return CALLOUT_TYPES[type] ? type : 'note';
}

function splitWikiTarget(inner: string): { target: string; alias: string } {
	const pipe = inner.indexOf('|');
	const raw = pipe < 0 ? inner : inner.slice(0, pipe);
	return {
		target: raw.split('#')[0].trim(),
		alias: (pipe < 0 ? raw : inner.slice(pipe + 1)).trim()
	};
}

function mathInline(katex: KatexLike) {
	return (state: StateInline, silent: boolean): boolean => {
		if (state.src.charCodeAt(state.pos) !== 0x24) return false;
		const next = state.src.charCodeAt(state.pos + 1);
		if (next === 0x24 || next === 0x20 || Number.isNaN(next)) return false;
		let end = state.src.indexOf('$', state.pos + 1);
		while (end > 0 && state.src.charCodeAt(end - 1) === 0x5c) end = state.src.indexOf('$', end + 1);
		if (end < 0) return false;
		const latex = state.src.slice(state.pos + 1, end);
		if (!latex.trim()) return false;
		if (!silent) {
			const token = state.push('html_inline', '', 0);
			token.content = `<span class="math-inline">${katex.renderToString(latex, { throwOnError: false })}</span>`;
		}
		state.pos = end + 1;
		return true;
	};
}

function mathBlock(katex: KatexLike) {
	return (state: StateBlock, startLine: number, endLine: number, silent: boolean): boolean => {
		const start = state.bMarks[startLine] + state.tShift[startLine];
		const max = state.eMarks[startLine];
		if (state.src.slice(start, start + 2) !== '$$') return false;
		if (silent) return true;
		let body = state.src.slice(start + 2, max);
		let nextLine = startLine;
		const inlineClose = body.indexOf('$$');
		if (inlineClose >= 0) {
			body = body.slice(0, inlineClose);
		} else {
			nextLine = startLine + 1;
			while (nextLine < endLine) {
				const line = state.src.slice(
					state.bMarks[nextLine] + state.tShift[nextLine],
					state.eMarks[nextLine]
				);
				const close = line.indexOf('$$');
				if (close >= 0) {
					body += `\n${line.slice(0, close)}`;
					break;
				}
				body += `\n${line}`;
				nextLine += 1;
			}
		}
		state.line = nextLine + 1;
		const token = state.push('html_block', '', 0);
		token.map = [startLine, state.line];
		token.content = `<div class="math-block">${katex.renderToString(body.trim(), {
			displayMode: true,
			throwOnError: false
		})}</div>\n`;
		return true;
	};
}

function markInline(state: StateInline, silent: boolean): boolean {
	if (state.src.charCodeAt(state.pos) !== 0x3d || state.src.charCodeAt(state.pos + 1) !== 0x3d)
		return false;
	const end = state.src.indexOf('==', state.pos + 2);
	if (end <= state.pos + 2) return false;
	if (!silent) {
		state.push('mark_open', 'mark', 1).markup = '==';
		state.push('text', '', 0).content = state.src.slice(state.pos + 2, end);
		state.push('mark_close', 'mark', -1).markup = '==';
	}
	state.pos = end + 2;
	return true;
}

function parseWiki(state: StateInline, from: number, closing: string): string | null {
	const end = state.src.indexOf(closing, from);
	if (end < 0) return null;
	const inner = state.src.slice(from, end).trim();
	if (!inner || inner.includes('\n') || inner.includes('[')) return null;
	return inner;
}

function wikiInline(state: StateInline, silent: boolean): boolean {
	if (state.src.charCodeAt(state.pos) !== 0x5b || state.src.charCodeAt(state.pos + 1) !== 0x5b)
		return false;
	const inner = parseWiki(state, state.pos + 2, ']]');
	if (inner === null) return false;
	const { target, alias } = splitWikiTarget(inner);
	if (!target) return false;
	if (!silent) {
		const token = state.push('wiki_link', 'a', 0);
		token.content = alias || target;
		token.attrSet('href', '#');
	}
	state.pos = state.pos + inner.length + 4;
	return true;
}

function embedInline(host: ResolveSources) {
	return (state: StateInline, silent: boolean): boolean => {
		if (state.src.charCodeAt(state.pos) !== 0x21 || state.src.charCodeAt(state.pos + 1) !== 0x5b)
			return false;
		const inner = parseWiki(state, state.pos + 3, ']]');
		if (inner === null) return false;
		const { target } = splitWikiTarget(inner);
		if (!target) return false;
		if (!silent) {
			const asset = resolveEmbed(target, host);
			const token = state.push('wiki_embed', 'img', 0);
			token.content = target;
			token.attrSet('src', asset ? buildLocalfileUrl(asset.abs) : imageSource(target, host));
		}
		state.pos = state.pos + inner.length + 5;
		return true;
	};
}

function colonCallout(
	state: StateBlock,
	startLine: number,
	endLine: number,
	silent: boolean
): boolean {
	const start = state.bMarks[startLine] + state.tShift[startLine];
	const max = state.eMarks[startLine];
	const match = COLON_CALLOUT.exec(state.src.slice(start, max));
	if (!match) return false;
	if (silent) return true;
	let nextLine = startLine + 1;
	while (nextLine < endLine) {
		const line = state.src.slice(
			state.bMarks[nextLine] + state.tShift[nextLine],
			state.eMarks[nextLine]
		);
		if (COLON_END.test(line.trim())) break;
		nextLine += 1;
	}
	const outerLineMax = state.lineMax;
	const outerParent = state.parentType;
	state.lineMax = nextLine;
	state.parentType = 'blockquote';
	const open = state.push('callout_open', 'div', 1);
	open.attrSet('class', `callout callout-${calloutType(match[1])}`);
	state.md.block.tokenize(state, startLine + 1, nextLine);
	state.push('callout_close', 'div', -1);
	state.lineMax = outerLineMax;
	state.parentType = outerParent;
	state.line = nextLine + 1;
	return true;
}

function stripCalloutMarker(tokens: Token[]): void {
	for (let i = 0; i < tokens.length; i++) {
		if (tokens[i].type !== 'blockquote_open') continue;
		const inline = tokens[i + 2];
		if (tokens[i + 1]?.type !== 'paragraph_open' || inline?.type !== 'inline') continue;
		const first = inline.children?.[0];
		if (!first || first.type !== 'text') continue;
		const match = MARKER.exec(first.content);
		if (!match) continue;
		first.content = first.content.slice(match[0].length);
		if (!first.content && inline.children?.[1]?.type === 'softbreak') inline.children.splice(0, 2);
		tokens[i].attrJoin('class', `callout callout-${calloutType(match[1])}`);
	}
}

function markTasks(state: StateCore): void {
	const tokens = state.tokens;
	const stack: Token[] = [];
	for (let i = 0; i < tokens.length; i++) {
		const token = tokens[i];
		if (token.nesting === 1) stack.push(token);
		else if (token.nesting === -1) stack.pop();
		if (token.type !== 'inline') continue;
		const paragraph = stack[stack.length - 1];
		const item = stack[stack.length - 2];
		if (paragraph?.type !== 'paragraph_open' || item?.type !== 'list_item_open') continue;
		const first = token.children?.[0];
		if (!first || first.type !== 'text') continue;
		const match = /^\[([ xX])\]\s+/.exec(first.content);
		if (!match) continue;
		first.content = first.content.slice(match[0].length);
		const box = new state.Token('html_inline', '', 0);
		box.content = `<input class="task-checkbox" type="checkbox" disabled${
			match[1].toLowerCase() === 'x' ? ' checked' : ''
		}>`;
		token.children?.unshift(box);
		item.attrJoin('class', 'task-item');
		for (let k = stack.length - 1; k >= 0; k--) {
			const open = stack[k];
			if (open.type === 'bullet_list_open' || open.type === 'ordered_list_open') {
				open.attrJoin('class', 'task-list');
				break;
			}
		}
	}
}

export function renderMarkdownToHtml(markdown: string, options: PdfRenderOptions): string {
	const { host, katex } = options;
	const md = new MarkdownIt({
		html: false,
		breaks: false,
		linkify: false,
		highlight: highlightCode
	});

	md.block.ruler.before('fence', 'margin_math_block', mathBlock(katex));
	md.block.ruler.before('blockquote', 'margin_colon_callout', colonCallout);
	md.inline.ruler.before('emphasis', 'margin_mark', markInline);
	md.inline.ruler.before('link', 'margin_embed', embedInline(host));
	md.inline.ruler.before('link', 'margin_wikilink', wikiInline);
	md.inline.ruler.before('link', 'margin_math', mathInline(katex));
	md.core.ruler.after('inline', 'margin_tasks', markTasks);
	md.core.ruler.after('inline', 'margin_callouts', (state) => stripCalloutMarker(state.tokens));

	md.renderer.rules.wiki_link = (tokens, idx) =>
		`<a class="wiki-link" href="#">${escapeHtml(tokens[idx].content)}</a>`;
	md.renderer.rules.wiki_embed = (tokens, idx) =>
		`<img class="wiki-embed" src="${escapeHtml(String(tokens[idx].attrGet('src') ?? ''))}" alt="${escapeHtml(
			tokens[idx].content
		)}">`;
	md.renderer.rules.math_inline = (tokens, idx) => tokens[idx].content;

	const defaultFence = md.renderer.rules.fence!;
	md.renderer.rules.fence = (tokens, idx, renderOptions, env, self) => {
		const token = tokens[idx];
		const lang = token.info.trim().split(/\s+/)[0];
		if (lang === 'mermaid')
			return `<div data-type="mermaid" data-mermaid="${escapeHtml(token.content)}"></div>\n`;
		return defaultFence(tokens, idx, renderOptions, env, self);
	};

	const defaultImage = md.renderer.rules.image!;
	md.renderer.rules.image = (tokens, idx, renderOptions, env, self) => {
		const src = tokens[idx].attrGet('src');
		if (src) tokens[idx].attrSet('src', imageSource(String(src), host));
		return defaultImage(tokens, idx, renderOptions, env, self);
	};

	return md.render(markdown);
}
