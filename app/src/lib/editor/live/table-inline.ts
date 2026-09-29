import { bareLinkAt } from '$lib/utils/web-link';
import type { ImageRef } from './assets';
import { paintMath } from './math-render';
import { INLINE_HTML, LINE_BREAK } from './html-inline';
import { imageElement } from './widgets';

export type ImageLookup = (target: string, embed: boolean, label: string) => ImageRef | null;

export interface CellLookups {
	image?: ImageLookup;
	footnote?: (id: string) => number | undefined;
}

interface Marker {
	open: string;
	close: string;
	tag: string;
	cls: string;
}

const MARKERS: Marker[] = [
	{ open: '**', close: '**', tag: 'strong', cls: 'cm-lp-strong' },
	{ open: '__', close: '__', tag: 'strong', cls: 'cm-lp-strong' },
	{ open: '~~', close: '~~', tag: 'del', cls: 'cm-lp-strike' },
	{ open: '==', close: '==', tag: 'mark', cls: 'cm-lp-highlight' },
	{ open: '*', close: '*', tag: 'em', cls: 'cm-lp-em' },
	{ open: '_', close: '_', tag: 'em', cls: 'cm-lp-em' }
];

const TAG_NAME = /^[\p{L}\p{N}_/-]+/u;
const TAG_LETTER = /[\p{L}_]/u;
const FOOTNOTE = /^\[\^([^\]\s]+)\]/;

function element(doc: Document, tag: string, cls: string): HTMLElement {
	const el = doc.createElement(tag);
	el.className = cls;
	return el;
}

function wikiAt(text: string, at: number): { label: string; target: string; end: number } | null {
	if (!text.startsWith('[[', at)) return null;
	const close = text.indexOf(']]', at + 2);
	if (close < 0) return null;
	const inner = text.slice(at + 2, close);
	const pipe = inner.indexOf('|');
	const path = pipe < 0 ? inner : inner.slice(0, pipe);
	const label = pipe < 0 ? path.replace(/^#/, '').replace(/#/g, ' > ') : inner.slice(pipe + 1);
	return { label: label || inner, target: path.trim(), end: close + 2 };
}

function mathAt(text: string, at: number): { tex: string; end: number } | null {
	if (text[at] !== '$' || text[at + 1] === '$' || /\s/.test(text[at + 1] ?? ' ')) return null;
	for (let close = at + 2; close < text.length; close++) {
		if (text[close] !== '$' || text[close - 1] === '\\' || /\s/.test(text[close - 1])) continue;
		if (/\d/.test(text[close + 1] ?? '')) return null;
		return { tex: text.slice(at + 1, close), end: close + 1 };
	}
	return null;
}

function tagAt(text: string, at: number): { tag: string; end: number } | null {
	if (text[at] !== '#' || (at > 0 && !/[\s(>[\]]/.test(text[at - 1]))) return null;
	const name = TAG_NAME.exec(text.slice(at + 1))?.[0].replace(/\/+$/, '');
	if (!name || !TAG_LETTER.test(name)) return null;
	return { tag: text.slice(at, at + 1 + name.length), end: at + 1 + name.length };
}

function codeAt(text: string, at: number): { code: string; end: number } | { run: string } {
	const run = /^`+/.exec(text.slice(at))![0];
	const close = new RegExp(`(?<!\`)${run}(?!\`)`, 'g');
	close.lastIndex = at + run.length;
	const match = close.exec(text);
	if (!match) return { run };
	const code = text.slice(at + run.length, match.index);
	return {
		code: /^ .*[^ ].* $/s.test(code) ? code.slice(1, -1) : code,
		end: match.index + run.length
	};
}

function htmlAt(text: string, at: number): { name: string; inner: string; end: number } | null {
	const name = /^<([a-z]+)>/i.exec(text.slice(at))?.[1].toLowerCase();
	if (!name || !INLINE_HTML[name]) return null;
	const close = text.toLowerCase().indexOf(`</${name}>`, at + name.length + 2);
	if (close < 0) return null;
	return { name, inner: text.slice(at + name.length + 2, close), end: close + name.length + 3 };
}

function linkAt(text: string, at: number): { label: string; href: string; end: number } | null {
	if (text[at] !== '[') return null;
	const close = text.indexOf(']', at + 1);
	if (close < 0 || text[close + 1] !== '(') return null;
	const end = text.indexOf(')', close + 2);
	if (end < 0) return null;
	return {
		label: text.slice(at + 1, close),
		href: text.slice(close + 2, end).trim(),
		end: end + 1
	};
}

function renderInto(doc: Document, parent: Node, text: string, lookups: CellLookups): void {
	const { image } = lookups;
	let plain = '';
	let at = 0;
	const flush = () => {
		if (!plain) return;
		parent.appendChild(doc.createTextNode(plain));
		plain = '';
	};

	while (at < text.length) {
		const char = text[at];
		if (char === '\\' && at + 1 < text.length) {
			plain += text[at + 1];
			at += 2;
			continue;
		}

		const lineBreak = char === '<' ? LINE_BREAK.exec(text.slice(at)) : null;
		if (lineBreak) {
			flush();
			parent.appendChild(doc.createElement('br'));
			at += lineBreak[0].length;
			continue;
		}

		const html = char === '<' ? htmlAt(text, at) : null;
		if (html) {
			flush();
			const el = element(doc, 'span', INLINE_HTML[html.name]);
			renderInto(doc, el, html.inner, lookups);
			parent.appendChild(el);
			at = html.end;
			continue;
		}

		if (char === '!' && text[at + 1] === '[' && image) {
			const wiki = wikiAt(text, at + 1);
			const link = wiki ? null : linkAt(text, at + 1);
			const found = wiki
				? image(text.slice(at + 3, wiki.end - 2), true, '')
				: link
					? image(link.href, false, link.label)
					: null;
			const end = wiki?.end ?? link?.end;
			if (found && end) {
				flush();
				parent.appendChild(imageElement(doc, found));
				at = end;
				continue;
			}
			if (wiki) {
				flush();
				const el = element(doc, 'span', 'cm-lp-wikilink');
				el.textContent = wiki.target;
				el.dataset.wiki = wiki.target;
				parent.appendChild(el);
				at = wiki.end;
				continue;
			}
		}

		const bare = bareLinkAt(text, at);
		if (bare) {
			flush();
			const el = element(doc, 'span', 'cm-lp-link');
			el.textContent = bare.label;
			el.dataset.href = bare.href;
			parent.appendChild(el);
			at = bare.end;
			continue;
		}

		if (char === '[') {
			const wiki = wikiAt(text, at);
			if (wiki) {
				flush();
				const el = element(doc, 'span', 'cm-lp-wikilink');
				el.textContent = wiki.label;
				if (wiki.target) el.dataset.wiki = wiki.target;
				parent.appendChild(el);
				at = wiki.end;
				continue;
			}
			const link = linkAt(text, at);
			if (link) {
				flush();
				const el = element(doc, 'span', 'cm-lp-link');
				if (link.href) el.dataset.href = link.href;
				renderInto(doc, el, link.label, lookups);
				parent.appendChild(el);
				at = link.end;
				continue;
			}
			const footnote = FOOTNOTE.exec(text.slice(at));
			const number = footnote && lookups.footnote?.(footnote[1]);
			if (footnote) {
				flush();
				const sup = element(doc, 'sup', 'cm-lp-footnote-ref');
				const label = sup.appendChild(element(doc, 'span', 'cm-lp-footnote-link'));
				label.textContent = number ? String(number) : footnote[1];
				parent.appendChild(sup);
				at += footnote[0].length;
				continue;
			}
		}

		const math = mathAt(text, at);
		if (math) {
			flush();
			const el = element(doc, 'span', 'cm-lp-math-inline');
			paintMath(el, math.tex, false);
			parent.appendChild(el);
			at = math.end;
			continue;
		}

		const tag = tagAt(text, at);
		if (tag) {
			flush();
			const el = element(doc, 'span', 'cm-lp-tag');
			el.textContent = tag.tag;
			el.dataset.tag = tag.tag.slice(1);
			parent.appendChild(el);
			at = tag.end;
			continue;
		}

		if (char === '`') {
			const span = codeAt(text, at);
			if ('run' in span) {
				plain += span.run;
				at += span.run.length;
				continue;
			}
			flush();
			const el = element(doc, 'code', 'cm-lp-code-inline');
			el.textContent = span.code;
			parent.appendChild(el);
			at = span.end;
			continue;
		}

		const marker = MARKERS.find((candidate) => text.startsWith(candidate.open, at));
		if (marker) {
			const close = text.indexOf(marker.close, at + marker.open.length);
			if (close > at) {
				flush();
				const el = element(doc, marker.tag, marker.cls);
				const inner = text.slice(at + marker.open.length, close);
				renderInto(doc, el, inner, lookups);
				parent.appendChild(el);
				at = close + marker.close.length;
				continue;
			}
		}

		plain += char;
		at++;
	}
	flush();
}

export function renderCell(text: string, lookups: CellLookups = {}): DocumentFragment {
	const fragment = document.createDocumentFragment();
	renderInto(document, fragment, text, lookups);
	return fragment;
}
