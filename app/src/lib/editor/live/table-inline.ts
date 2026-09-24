interface Marker {
	open: string;
	close: string;
	tag: string;
	cls: string;
}

const MARKERS: Marker[] = [
	{ open: '`', close: '`', tag: 'code', cls: 'cm-lp-code-inline' },
	{ open: '**', close: '**', tag: 'strong', cls: 'cm-lp-strong' },
	{ open: '__', close: '__', tag: 'strong', cls: 'cm-lp-strong' },
	{ open: '~~', close: '~~', tag: 'del', cls: 'cm-lp-strike' },
	{ open: '==', close: '==', tag: 'mark', cls: 'cm-lp-highlight' },
	{ open: '*', close: '*', tag: 'em', cls: 'cm-lp-em' },
	{ open: '_', close: '_', tag: 'em', cls: 'cm-lp-em' }
];

function element(doc: Document, tag: string, cls: string): HTMLElement {
	const el = doc.createElement(tag);
	el.className = cls;
	return el;
}

function wikiAt(text: string, at: number): { label: string; end: number } | null {
	if (!text.startsWith('[[', at)) return null;
	const close = text.indexOf(']]', at + 2);
	if (close < 0) return null;
	const inner = text.slice(at + 2, close);
	const label = (inner.split('|').pop() ?? inner).split('#')[0];
	return { label: label || inner, end: close + 2 };
}

function linkAt(text: string, at: number): { label: string; end: number } | null {
	if (text[at] !== '[') return null;
	const close = text.indexOf(']', at + 1);
	if (close < 0 || text[close + 1] !== '(') return null;
	const end = text.indexOf(')', close + 2);
	if (end < 0) return null;
	return { label: text.slice(at + 1, close), end: end + 1 };
}

function renderInto(doc: Document, parent: Node, text: string): void {
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

		if (char === '[') {
			const wiki = wikiAt(text, at);
			if (wiki) {
				flush();
				const el = element(doc, 'span', 'cm-lp-wikilink');
				el.textContent = wiki.label;
				parent.appendChild(el);
				at = wiki.end;
				continue;
			}
			const link = linkAt(text, at);
			if (link) {
				flush();
				const el = element(doc, 'span', 'cm-lp-link');
				renderInto(doc, el, link.label);
				parent.appendChild(el);
				at = link.end;
				continue;
			}
		}

		const marker = MARKERS.find((candidate) => text.startsWith(candidate.open, at));
		if (marker) {
			const close = text.indexOf(marker.close, at + marker.open.length);
			if (close > at) {
				flush();
				const el = element(doc, marker.tag, marker.cls);
				const inner = text.slice(at + marker.open.length, close);
				if (marker.open === '`') el.textContent = inner;
				else renderInto(doc, el, inner);
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

export function renderCell(text: string): DocumentFragment {
	const fragment = document.createDocumentFragment();
	renderInto(document, fragment, text);
	return fragment;
}
