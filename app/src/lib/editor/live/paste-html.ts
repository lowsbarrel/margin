import type TurndownService from 'turndown';
import { markdownLink } from '$lib/utils/web-link';
import type { Align } from './table-model';

const CODE_VIEW_MARKERS =
	/monaco-editor|cm-editor|CodeMirror|blob-code|blob-wrapper|highlight-source|linenumbers|line-numbers|hljs/;
const MONOSPACE = /monospace|consolas|courier|menlo|monaco|source code|jetbrains mono|sf mono/i;
const BLOCK_LEAD = /^([ \t]{0,3})(-{2,}|={2,}|#{1,6}|[-+*]|\d+[.)]|>)(?=[ \t]|$)/gm;
const DROPPED = 'style, script, meta, title, link, noscript, template, head';
const BOLD = /font-weight\s*:\s*(bold|[6-9]00)/i;
const PLAIN_WEIGHT = /font-weight\s*:\s*(normal|[1-4]00)/i;
const ITALIC = /font-style\s*:\s*italic/i;
const STRIKE = /text-decoration[^;]*line-through/i;
const WORD_LIST = /mso-list\s*:\s*l\d+\s+level(\d+)/i;
const WORD_MARKER = /^[\s\u00a0]*(?:[·•o§▪\-–]|(\w{1,4})[.)])[\s\u00a0]*$/;
const STRUCK: Record<string, true> = { DEL: true, S: true, STRIKE: true };
const RULE: Record<Align, string> = { left: ':--', center: ':-:', right: '--:', none: '---' };

// A code editor's clipboard HTML is a syntax-highlighted dump: turning it into Markdown would mangle the source.
export function isCodeEditorHtml(html: string): boolean {
	const body = new DOMParser().parseFromString(html, 'text/html').body;
	const root = body.firstElementChild;
	if (root && /white-space\s*:\s*(pre|pre-wrap|pre-line)/.test(root.getAttribute('style') ?? ''))
		return true;
	if (CODE_VIEW_MARKERS.test(html)) return true;
	const runs = [...body.querySelectorAll('*')].filter(
		(el) => el.children.length === 0 && (el.textContent ?? '').trim().length > 0
	);
	if (runs.length === 0) return false;
	return runs.every(
		(el) =>
			MONOSPACE.test(el.getAttribute('style') ?? '') ||
			MONOSPACE.test(el.parentElement?.getAttribute('style') ?? '')
	);
}

function escapeMarkdown(text: string): string {
	return text
		.replace(/[\\*`[\]]/g, '\\$&')
		.replace(/(^|\W)_|_(?=\W|$)/g, (match) => match.replace('_', '\\_'))
		.replace(BLOCK_LEAD, '$1\\$2');
}

function unwrap(el: Element): void {
	el.replaceWith(...el.childNodes);
}

function wrapChildren(el: Element, tag: string): void {
	const wrapper = el.ownerDocument.createElement(tag);
	wrapper.append(...el.childNodes);
	el.append(wrapper);
}

function styledEmphasis(body: HTMLElement): void {
	for (const el of body.querySelectorAll('b, strong'))
		if (PLAIN_WEIGHT.test(el.getAttribute('style') ?? '')) unwrap(el);
	for (const el of body.querySelectorAll('span[style]')) {
		if (el.closest('h1, h2, h3, h4, h5, h6, th, pre, code, b, strong')) continue;
		if (!(el.textContent ?? '').trim()) continue;
		const style = el.getAttribute('style') ?? '';
		if (STRIKE.test(style)) wrapChildren(el, 'del');
		if (ITALIC.test(style)) wrapChildren(el, 'em');
		if (BOLD.test(style)) wrapChildren(el, 'strong');
	}
}

function wordListMarker(p: Element): { ordered: boolean } | null {
	for (const span of p.querySelectorAll('span')) {
		const match = WORD_MARKER.exec(span.textContent ?? '');
		if (!match) continue;
		span.remove();
		return { ordered: match[1] !== undefined };
	}
	return null;
}

// Word exports list items as paragraphs styled `mso-list` with the bullet typed into a span.
function wordLists(body: HTMLElement): void {
	const stack: Element[] = [];
	for (const p of [...body.querySelectorAll('p')]) {
		const level = WORD_LIST.exec(p.getAttribute('style') ?? '');
		const marker = level && wordListMarker(p);
		if (!level || !marker) continue;
		if (p.previousElementSibling !== stack[0]) stack.length = 0;
		const depth = Number(level[1]);
		while (stack.length > depth) stack.pop();
		while (stack.length < depth) {
			const list = p.ownerDocument.createElement(marker.ordered ? 'ol' : 'ul');
			const owner = stack.at(-1)?.lastElementChild;
			if (owner) owner.append(list);
			else p.before(list);
			stack.push(list);
		}
		const item = p.ownerDocument.createElement('li');
		item.append(...p.childNodes);
		stack[depth - 1].append(item);
		p.remove();
	}
}

function prepare(html: string): HTMLElement {
	const body = new DOMParser().parseFromString(html, 'text/html').body;
	for (const el of body.querySelectorAll(DROPPED)) el.remove();
	for (const el of body.querySelectorAll('a[href^="#"]')) unwrap(el);
	wordLists(body);
	styledEmphasis(body);
	return body;
}

function listItem(content: string, item: HTMLElement, options: TurndownService.Options): string {
	const body = content
		.replace(/^\s+|\s+$/g, '')
		.replace(/^(\[[ x]\]) +/, '$1 ')
		.replace(/\n/gm, '\n    ');
	const parent = item.parentNode as HTMLElement | null;
	let prefix = `${options.bulletListMarker} `;
	if (parent?.nodeName === 'OL') {
		const start = parent.getAttribute('start');
		const index = Array.prototype.indexOf.call(parent.children, item);
		prefix = `${start ? Number(start) + index : index + 1}. `;
	}
	return prefix + body + (item.nextSibling ? '\n' : '');
}

function alignOf(cell: HTMLTableCellElement): Align {
	const value = (cell.getAttribute('align') ?? cell.style.textAlign).toLowerCase();
	return value === 'left' || value === 'center' || value === 'right' ? value : 'none';
}

function tableMarkdown(service: TurndownService, table: HTMLTableElement): string {
	const rows: string[][] = [];
	for (const row of table.rows) {
		const cells: string[] = [];
		for (const cell of row.cells) {
			const text = service
				.turndown(cell.innerHTML)
				.trim()
				.replace(/\n+/g, '<br>')
				.replace(/(?<!\\)\|/g, '\\|');
			cells.push(text, ...Array<string>(Math.max(0, cell.colSpan - 1)).fill(''));
		}
		if (cells.some(Boolean)) rows.push(cells);
	}
	if (rows.length === 0) return '';
	const cols = Math.max(...rows.map((cells) => cells.length));
	if (rows.length === 1 && cols === 1) return `\n\n${rows[0][0]}\n\n`;
	for (const cells of rows) while (cells.length < cols) cells.push('');
	const head = table.rows[0];
	const rule = Array.from(
		{ length: cols },
		(_, col) => RULE[head.cells[col] ? alignOf(head.cells[col]) : 'none']
	);
	const lines = [rows[0], rule, ...rows.slice(1)].map((cells) => `|${cells.join('|')}|`);
	return `\n\n${lines.join('\n')}\n\n`;
}

export async function htmlToMarkdown(html: string): Promise<string> {
	const [{ default: TurndownService }, { highlightedCodeBlock, taskListItems }] = await Promise.all(
		[import('turndown'), import('turndown-plugin-gfm')]
	);
	const service = new TurndownService({
		headingStyle: 'atx',
		hr: '---',
		br: '',
		bulletListMarker: '-',
		codeBlockStyle: 'fenced',
		emDelimiter: '_',
		strongDelimiter: '**'
	});
	service.use([highlightedCodeBlock, taskListItems]);
	service.addRule('listItem', { filter: 'li', replacement: listItem });
	service.addRule('strikethrough', {
		filter: (node) => STRUCK[node.nodeName] === true,
		replacement: (content) => (content.trim() ? `~~${content}~~` : content)
	});
	service.addRule('link', {
		filter: (node) => node.nodeName === 'A' && !!node.getAttribute('href'),
		replacement: (content, node) => {
			const href = (node as HTMLElement).getAttribute('href') ?? '';
			return content.trim() ? markdownLink(content, href.replace(/ /g, '%20')) : '';
		}
	});
	service.addRule('table', {
		filter: 'table',
		replacement: (_content, node) => tableMarkdown(service, node as HTMLTableElement)
	});
	service.escape = escapeMarkdown;
	return service
		.turndown(prepare(html))
		.replace(/\u00a0/g, ' ')
		.replace(/\n{3,}/g, '\n\n');
}
