import type { EditorView } from '@codemirror/view';
import * as m from '$lib/paraglide/messages.js';
import { toast } from '$lib/stores/toast.svelte';
import { WEB_URL } from '$lib/utils/web-link';
import { openHref } from './click';
import { contextOf } from './context';
import { asList, type PropertyKind, type PropertyValue } from './frontmatter';
import type { FocusTarget } from './property-edits';
import { strokeIcon } from './widgets';

export interface ValueHost {
	view: EditorView;
	key: string;
	kind: PropertyKind;
	value: PropertyValue;
	commit(value: PropertyValue, target: FocusTarget | null): void;
	leave(): void;
}

export interface ValueEditor {
	el: HTMLElement;
	focus(at: 'start' | 'end' | 'all'): void;
	focusPill?(index: number): void;
}

const X_ICON = '<path d="M18 6 6 18"/><path d="m6 6 12 12"/>';
const PENCIL_ICON =
	'<path d="M21.174 6.812a1 1 0 0 0-3.986-3.987L3.842 16.174a2 2 0 0 0-.5.83l-1.321 4.352a.5.5 0 0 0 .623.622l4.353-1.32a2 2 0 0 0 .83-.497z"/>';
const TAG_NAME = /^(?=.*[\p{L}_])[\p{L}\p{N}_/-]+$/u;

// WebKit's native select-all escapes a nested editing host and selects the whole note.
function editable(className: string, text: string): HTMLDivElement {
	const el = document.createElement('div');
	el.className = className;
	el.contentEditable = 'plaintext-only';
	el.tabIndex = 0;
	el.textContent = text;
	el.addEventListener('keydown', (event) => {
		if (event.key.toLowerCase() !== 'a' || !(event.metaKey || event.ctrlKey) || event.shiftKey)
			return;
		event.preventDefault();
		placeCaret(el, 'all');
	});
	return el;
}

function placeCaret(el: HTMLElement, at: 'start' | 'end' | 'all'): void {
	el.focus();
	const selection = window.getSelection();
	if (!selection) return;
	const range = document.createRange();
	range.selectNodeContents(el);
	if (at !== 'all') range.collapse(at === 'start');
	selection.removeAllRanges();
	selection.addRange(range);
}

function linkOf(view: EditorView, text: string): { label: string; open(): void } | null {
	const ctx = contextOf(view.state);
	const wiki = /^\[\[([^\]]+)\]\]$/.exec(text);
	if (wiki) {
		const [target, alias] = wiki[1].split('|');
		return { label: (alias ?? target).trim(), open: () => ctx.openWikiLink(target.trim()) };
	}
	const markdown = /^\[([^\]]*)\]\(<?([^)>]+)>?\)$/.exec(text);
	if (markdown) {
		const href = markdown[2].trim();
		const open = /^[a-z][a-z0-9+.-]*:/i.test(href)
			? () => openHref(href, ctx.vaultPath())
			: () => ctx.openWikiLink(decodeURI(href).replace(/\.md$/i, ''));
		return { label: markdown[1] || href, open };
	}
	return WEB_URL.test(text) ? { label: text, open: () => openHref(text, ctx.vaultPath()) } : null;
}

function textEditor(host: ValueHost): ValueEditor {
	const original = host.value === null ? '' : String(host.value);
	const wrap = document.createElement('div');
	wrap.className = 'cm-lp-property-field';
	const input = editable('cm-lp-property-text', original);
	input.dataset.placeholder = m.editor_property_empty();
	const link = linkOf(host.view, original);
	const linkView = document.createElement('div');
	const show = (editing: boolean) => wrap.replaceChildren(editing || !link ? input : linkView);
	if (link) {
		linkView.className = 'cm-lp-property-link';
		const label = document.createElement('span');
		label.className = 'cm-lp-wikilink';
		label.textContent = link.label;
		label.addEventListener('mousedown', (event) => event.preventDefault());
		label.addEventListener('click', (event) => {
			event.stopPropagation();
			link.open();
		});
		linkView.append(label, strokeIcon(PENCIL_ICON, 'cm-lp-property-flair'));
		linkView.addEventListener('click', () => {
			show(true);
			placeCaret(input, 'end');
		});
	}
	let done = false;
	const finish = (text: string, target: FocusTarget | null) => {
		if (done) return;
		if (text === original) {
			if (target) host.leave();
			else if (link) show(false);
			return;
		}
		done = true;
		host.commit(text, target);
	};
	input.addEventListener('keydown', (event) => {
		if (event.isComposing) return;
		if (event.key === 'Enter' && !event.shiftKey) {
			event.preventDefault();
			finish(input.textContent ?? '', { part: 'row', key: host.key });
		} else if (event.key === 'Escape') {
			event.preventDefault();
			input.textContent = original;
			host.leave();
		}
	});
	input.addEventListener('blur', () => finish((input.textContent ?? '').trimEnd(), null));
	show(false);
	return {
		el: wrap,
		focus(at) {
			show(true);
			placeCaret(input, at);
		}
	};
}

function fieldInput(host: ValueHost, type: string, value: string): ValueEditor {
	const input = document.createElement('input');
	input.type = type;
	input.className = `cm-lp-property-input cm-lp-property-${host.kind}`;
	input.placeholder = m.editor_property_empty();
	input.value = value;
	if (type === 'number') {
		input.step = 'any';
		input.inputMode = 'decimal';
	} else input.max = type === 'date' ? '9999-12-31' : '9999-12-31T23:59';
	const parse = (): PropertyValue | undefined => {
		const text = input.value.trim();
		if (!text) return null;
		if (type === 'number') return Number.isFinite(Number(text)) ? Number(text) : undefined;
		return type === 'date' ? text : /T\d\d:\d\d$/.test(text) ? `${text}:00` : text;
	};
	const save = (target: FocusTarget | null) => {
		const next = parse();
		if (input.value === value || next === undefined) {
			if (target) host.leave();
			return;
		}
		host.commit(next, target);
	};
	input.addEventListener('keydown', (event) => {
		if (event.isComposing) return;
		if (event.key === 'Enter') {
			event.preventDefault();
			save({ part: 'row', key: host.key });
		} else if (event.key === 'Escape') {
			event.preventDefault();
			input.value = value;
			host.leave();
		}
	});
	input.addEventListener('blur', () => save(null));
	return { el: input, focus: () => input.focus() };
}

function checkboxEditor(host: ValueHost): ValueEditor {
	const box = document.createElement('input');
	box.type = 'checkbox';
	box.className = 'cm-lp-checkbox cm-lp-property-checkbox';
	box.checked = host.value === true;
	box.indeterminate = host.value === null;
	box.addEventListener('change', () => host.commit(box.checked, { part: 'value', key: host.key }));
	box.addEventListener('keydown', (event) => {
		if (event.key !== 'Enter' && event.key !== 'Escape') return;
		event.preventDefault();
		host.leave();
	});
	return { el: box, focus: () => box.focus() };
}

function pillContent(host: ValueHost, value: string): HTMLElement {
	const content = document.createElement('span');
	content.className = 'cm-lp-pill-content';
	const ctx = contextOf(host.view.state);
	const link = host.kind === 'tags' ? null : linkOf(host.view, value);
	const tag = host.kind === 'tags' ? value.replace(/^#/, '') : null;
	content.textContent = tag ?? link?.label ?? value;
	const open = tag !== null ? () => ctx.openTag(tag) : link?.open;
	if (open) {
		content.classList.add(tag !== null ? 'cm-lp-pill-tag' : 'cm-lp-wikilink');
		content.addEventListener('click', (event) => {
			event.stopPropagation();
			open();
		});
	}
	return content;
}

function pillsEditor(host: ValueHost): ValueEditor {
	const values = asList(host.value);
	const root = document.createElement('div');
	root.className = 'cm-lp-pills';
	const input = editable('cm-lp-pill-input', '');
	if (!values.length) input.dataset.placeholder = m.editor_property_empty();
	const pills: HTMLElement[] = [];
	const at = (index: number): FocusTarget =>
		index < 0 ? { part: 'value', key: host.key } : { part: 'pill', key: host.key, index };
	const clean = (text: string) => (host.kind === 'list' ? text : text.trim().replace(/^#/, ''));
	const accept = (text: string, except: number): string | null => {
		const value = clean(text);
		if (!value.trim()) return null;
		if (host.kind === 'tags' && !TAG_NAME.test(value)) {
			toast.error(m.editor_property_tag_invalid());
			return null;
		}
		const dupe =
			host.kind === 'list' ? -1 : values.findIndex((v, i) => i !== except && clean(v) === value);
		if (dupe < 0) return value;
		pills[dupe].classList.remove('cm-lp-pill-duplicate');
		void pills[dupe].offsetWidth;
		pills[dupe].classList.add('cm-lp-pill-duplicate');
		return null;
	};
	const add = (text: string, target: FocusTarget | null) => {
		const value = accept(text, -1);
		if (value !== null) host.commit([...values, value], target);
	};
	const edit = (index: number) => {
		const field = editable('cm-lp-pill-input cm-lp-pill-editing', values[index]);
		let open = true;
		const close = (text: string | null) => {
			if (!open) return;
			open = false;
			const value = text === null ? null : accept(text, index);
			if (value === null) {
				field.replaceWith(pills[index]);
				pills[index].focus();
			} else
				host.commit(
					values.map((v, i) => (i === index ? value : v)),
					at(index)
				);
		};
		field.addEventListener('keydown', (event) => {
			if (event.isComposing) return;
			if (event.key === 'Enter' || event.key === 'Escape') event.preventDefault();
			if (event.key === 'Enter') close(field.textContent ?? '');
			if (event.key === 'Escape') close(null);
		});
		field.addEventListener('blur', () => close(field.textContent || null));
		pills[index].replaceWith(field);
		placeCaret(field, 'all');
	};
	const without = (index: number) => values.filter((_, i) => i !== index);
	const focusPill = (index: number) => (pills[index] ?? input).focus();
	const moves: Record<string, (index: number) => void> = {
		Enter: edit,
		Escape: () => input.focus(),
		Backspace: (index) => host.commit(without(index), at(index - 1)),
		ArrowUp: () => focusPill(0),
		ArrowDown: () => input.focus(),
		ArrowLeft: (index) => focusPill(Math.max(0, index - 1)),
		ArrowRight: (index) => focusPill(index + 1)
	};
	values.forEach((value, index) => {
		const pill = document.createElement('div');
		pill.className = 'cm-lp-pill';
		pill.tabIndex = 0;
		const remove = document.createElement('span');
		remove.className = 'cm-lp-pill-remove';
		remove.appendChild(strokeIcon(X_ICON, 'cm-lp-pill-x', '2'));
		remove.addEventListener('mousedown', (event) => event.preventDefault());
		remove.addEventListener('click', (event) => {
			event.stopPropagation();
			host.commit(without(index), null);
		});
		pill.append(pillContent(host, value), remove);
		pill.addEventListener('dblclick', (event) => {
			event.preventDefault();
			edit(index);
		});
		pill.addEventListener('keydown', (event) => {
			const move = moves[event.key];
			if (!move || event.isComposing) return;
			event.preventDefault();
			move(index);
		});
		pills.push(pill);
	});
	input.addEventListener('keydown', (event) => {
		if (event.isComposing) return;
		const text = input.textContent ?? '';
		const atStart = window.getSelection()?.getRangeAt(0).startOffset === 0;
		if (event.key === 'Enter' && text) {
			event.preventDefault();
			add(text, { part: 'value', key: host.key });
		} else if (event.key === 'Enter' || event.key === 'Escape') {
			event.preventDefault();
			host.leave();
		} else if (
			(event.key === 'Backspace' || (event.key === 'ArrowLeft' && atStart)) &&
			!text &&
			pills.length
		) {
			event.preventDefault();
			focusPill(pills.length - 1);
		}
	});
	input.addEventListener('blur', () => {
		const text = input.textContent ?? '';
		if (text) add(text, null);
	});
	root.addEventListener('click', (event) => {
		if (event.target === root) input.focus();
	});
	root.append(...pills, input);
	return { el: root, focus: () => placeCaret(input, 'end'), focusPill };
}

function dateText(value: PropertyValue, kind: PropertyKind): string {
	if (typeof value !== 'string') return '';
	return kind === 'date' ? value.slice(0, 10) : value.replace(' ', 'T');
}

export function valueEditor(host: ValueHost): ValueEditor {
	const { kind, value } = host;
	if (kind === 'list' || kind === 'tags' || kind === 'aliases') return pillsEditor(host);
	if (kind === 'checkbox') return checkboxEditor(host);
	if (kind === 'number')
		return fieldInput(
			host,
			'number',
			typeof value === 'number' && Number.isFinite(value) ? String(value) : ''
		);
	if (kind === 'date' || kind === 'datetime')
		return fieldInput(host, kind === 'date' ? 'date' : 'datetime-local', dateText(value, kind));
	return textEditor(host);
}
