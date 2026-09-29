export const WEB_IMAGE = /^(?:https?:\/\/|data:image\/)/i;

export const WEB_URL = /^https?:\/\/[^\s<>]+$/i;
const BARE = /^(?:(?:https?:\/\/|www\.)[^\s<]+|[\w.+-]+@[\w-]+(?:\.[\w-]+)+)/i;
const TRAILING = /[?!.,:;*_~'"]+$/;
const ANGLE = /^<((?:[a-z][\w+.-]*:[^\s<>]*)|[\w.+-]+@[\w-]+(?:\.[\w-]+)+)>/i;

export interface FoundLink {
	label: string;
	href: string;
	end: number;
}

export function webHref(text: string): string {
	const raw = text.replace(/^<|>$/g, '');
	if (/^[a-z][\w+.-]*:/i.test(raw)) return raw;
	if (raw.includes('@') && !raw.includes('/')) return `mailto:${raw}`;
	return `https://${raw}`;
}

function count(text: string, char: string): number {
	return text.split(char).length - 1;
}

// GFM's extended autolinks drop trailing punctuation and any `)` without a matching `(`.
export function bareLinkAt(text: string, at: number): FoundLink | null {
	if (at > 0 && !/[\s(*_~]/.test(text[at - 1])) return null;
	const angle = text[at] === '<' ? ANGLE.exec(text.slice(at)) : null;
	if (angle) return { label: angle[1], href: webHref(angle[1]), end: at + angle[0].length };
	const match = BARE.exec(text.slice(at));
	if (!match) return null;
	let label = match[0].replace(TRAILING, '');
	while (label.endsWith(')') && count(label, ')') > count(label, '('))
		label = label.slice(0, -1).replace(TRAILING, '');
	if (!/[.:]/.test(label.replace(/^www\./i, ''))) return null;
	return { label, href: webHref(label), end: at + label.length };
}

export function markdownLink(label: string, url: string): string {
	const safe = count(url, '(') === count(url, ')') && !/[<>]/.test(url);
	return `[${label}](${safe ? url : `<${url}>`})`;
}
