import * as m from '$lib/paraglide/messages.js';

export type CalloutKind =
	| 'note'
	| 'info'
	| 'tip'
	| 'success'
	| 'question'
	| 'warning'
	| 'failure'
	| 'danger'
	| 'bug'
	| 'example'
	| 'quote';

const ALIASES: Record<string, CalloutKind> = {
	note: 'note',
	abstract: 'note',
	summary: 'note',
	tldr: 'note',
	info: 'info',
	todo: 'info',
	tip: 'tip',
	hint: 'tip',
	important: 'tip',
	success: 'success',
	check: 'success',
	done: 'success',
	question: 'question',
	help: 'question',
	faq: 'question',
	warning: 'warning',
	caution: 'warning',
	attention: 'warning',
	failure: 'failure',
	fail: 'failure',
	missing: 'failure',
	danger: 'danger',
	error: 'danger',
	bug: 'bug',
	example: 'example',
	quote: 'quote',
	cite: 'quote'
};

const ICONS: Record<CalloutKind, string> = {
	note: '<path d="M15 3H6a2 2 0 0 0-2 2v14a2 2 0 0 0 2 2h12a2 2 0 0 0 2-2V8Z"/><path d="M14 3v5h5"/>',
	info: '<circle cx="12" cy="12" r="9"/><path d="M12 16v-4"/><path d="M12 8h.01"/>',
	tip: '<path d="M9 18h6"/><path d="M10 22h4"/><path d="M12 2a7 7 0 0 0-4 12.7V17h8v-2.3A7 7 0 0 0 12 2Z"/>',
	success: '<circle cx="12" cy="12" r="9"/><path d="m8.5 12.5 2.5 2.5 4.5-5"/>',
	question:
		'<circle cx="12" cy="12" r="9"/><path d="M9.4 9a2.8 2.8 0 1 1 3.9 2.6c-.8.4-1.3 1.1-1.3 2"/><path d="M12 17h.01"/>',
	warning:
		'<path d="M10.3 4 2 18a2 2 0 0 0 1.7 3h16.6A2 2 0 0 0 22 18L13.7 4a2 2 0 0 0-3.4 0Z"/><path d="M12 9v4"/><path d="M12 17h.01"/>',
	failure: '<circle cx="12" cy="12" r="9"/><path d="m15 9-6 6"/><path d="m9 9 6 6"/>',
	danger: '<path d="M8 2h8l6 6v8l-6 6H8l-6-6V8Z"/><path d="M12 8v4"/><path d="M12 16h.01"/>',
	bug: '<rect x="8" y="7" width="8" height="11" rx="4"/><path d="M12 7V4.5"/><path d="M6 10H3"/><path d="M21 10h-3"/><path d="M6 15.5H3"/><path d="M21 15.5h-3"/><path d="m8.5 6-2-2"/><path d="m15.5 6 2-2"/>',
	example:
		'<path d="M9 6h11"/><path d="M9 12h11"/><path d="M9 18h11"/><path d="M4 6h.01"/><path d="M4 12h.01"/><path d="M4 18h.01"/>',
	quote:
		'<path d="M10 11H6.5A1.5 1.5 0 0 1 5 9.5V7a1 1 0 0 1 1-1h3a1 1 0 0 1 1 1v6a4 4 0 0 1-4 4"/><path d="M20 11h-3.5A1.5 1.5 0 0 1 15 9.5V7a1 1 0 0 1 1-1h3a1 1 0 0 1 1 1v6a4 4 0 0 1-4 4"/>'
};

export const CALLOUT_LABELS: Record<CalloutKind, () => string> = {
	note: m.callout_type_note,
	info: m.callout_type_info,
	tip: m.callout_type_tip,
	success: m.callout_type_success,
	question: m.callout_type_question,
	warning: m.callout_type_warning,
	failure: m.callout_type_failure,
	danger: m.callout_type_danger,
	bug: m.callout_type_bug,
	example: m.callout_type_example,
	quote: m.callout_type_quote
};

const SVG_NS = 'http://www.w3.org/2000/svg';

export function calloutKind(type: string): CalloutKind {
	return ALIASES[type.trim().toLowerCase()] ?? 'info';
}

export function calloutIcon(kind: CalloutKind): SVGSVGElement {
	const svg = document.createElementNS(SVG_NS, 'svg');
	svg.setAttribute('viewBox', '0 0 24 24');
	svg.setAttribute('fill', 'none');
	svg.setAttribute('stroke', 'currentColor');
	svg.setAttribute('stroke-width', '1.8');
	svg.setAttribute('stroke-linecap', 'round');
	svg.setAttribute('stroke-linejoin', 'round');
	svg.setAttribute('aria-hidden', 'true');
	svg.classList.add('cm-lp-callout-svg');
	svg.innerHTML = ICONS[kind];
	return svg;
}

export function chevronElement(collapsed: boolean): SVGSVGElement {
	const svg = document.createElementNS(SVG_NS, 'svg');
	svg.setAttribute('viewBox', '0 0 24 24');
	svg.setAttribute('fill', 'none');
	svg.setAttribute('stroke', 'currentColor');
	svg.setAttribute('stroke-width', '2.4');
	svg.setAttribute('stroke-linecap', 'round');
	svg.setAttribute('stroke-linejoin', 'round');
	svg.setAttribute('aria-hidden', 'true');
	svg.classList.add('cm-lp-callout-chevron');
	if (!collapsed) svg.classList.add('cm-lp-callout-chevron-open');
	const path = document.createElementNS(SVG_NS, 'path');
	path.setAttribute('d', 'm9 6 6 6-6 6');
	svg.appendChild(path);
	return svg;
}
