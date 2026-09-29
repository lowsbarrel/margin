import { redo, undo } from '@codemirror/commands';
import { WidgetType, type EditorView } from '@codemirror/view';
import * as m from '$lib/paraglide/messages.js';
import { contextOf } from './context';
import type { Property, PropertyKind } from './frontmatter';
import {
	addProperty,
	focusBody,
	registerFocuser,
	removeProperties,
	renameProperty,
	setPropertiesUi,
	setPropertyValue,
	type FocusTarget
} from './property-edits';
import { valueEditor, type ValueEditor } from './property-inputs';
import {
	checkKey,
	openPropertiesMenu,
	openPropertyMenu,
	PROPERTY_ICONS as ICONS
} from './property-menus';
import { strokeIcon } from './widgets';

export interface PanelEntry {
	property: Property;
	kind: PropertyKind;
}

interface Row {
	row: HTMLElement;
	keyInput: HTMLInputElement;
	editor: ValueEditor | null;
	signature: string;
}

function element<K extends keyof HTMLElementTagNameMap>(
	tag: K,
	className: string
): HTMLElementTagNameMap[K] {
	const el = document.createElement(tag);
	el.className = className;
	return el;
}

class Panel {
	readonly root = element('div', 'cm-lp-properties');
	private readonly heading = element('div', 'cm-lp-properties-heading');
	private readonly list = element('div', 'cm-lp-properties-list');
	private readonly add = element('div', 'cm-lp-property-add');
	private rows = new Map<string, Row>();
	private pending: HTMLElement | null = null;
	private collapsed = false;

	constructor(private readonly view: EditorView) {
		const { root, heading, add } = this;
		root.contentEditable = 'false';
		heading.tabIndex = 0;
		const title = element('span', 'cm-lp-properties-title');
		title.textContent = m.editor_properties();
		heading.append(strokeIcon(ICONS.fold, 'cm-lp-properties-fold', '2'), title);
		heading.addEventListener('mousedown', (event) => event.preventDefault());
		heading.addEventListener('click', () => this.fold(!this.collapsed));
		heading.addEventListener('contextmenu', (event) => this.headingMenu(event));
		heading.addEventListener('keydown', (event) => this.headingKey(event));
		add.tabIndex = 0;
		const label = element('span', 'cm-lp-property-add-label');
		label.textContent = m.editor_property_add();
		add.append(strokeIcon(ICONS.plus, 'cm-lp-property-add-icon'), label);
		add.addEventListener('click', () => this.startAdding());
		add.addEventListener('keydown', (event) => this.addKey(event));
		root.append(heading, this.list, add);
		registerFocuser(root, (target) => this.focus(target));
	}

	sync(entries: readonly PanelEntry[], collapsed: boolean, adding: boolean): void {
		this.collapsed = collapsed;
		this.root.classList.toggle('cm-lp-properties-collapsed', collapsed);
		this.root.dataset.count = String(entries.length);
		const next = new Map<string, Row>();
		for (const { property, kind } of entries) {
			const signature = JSON.stringify([kind, property.value, property.raw]);
			const current = this.rows.get(property.key);
			next.set(
				property.key,
				current?.signature === signature ? current : this.row(property, kind, signature)
			);
		}
		const wanted = [...next.values()].map((entry) => entry.row);
		if (this.pending) wanted.push(this.pending);
		for (const child of [...this.list.children])
			if (!wanted.includes(child as HTMLElement)) child.remove();
		wanted.forEach((row, index) => {
			if (this.list.children[index] !== row)
				this.list.insertBefore(row, this.list.children[index] ?? null);
		});
		this.rows = next;
		if (adding && !this.pending) this.startAdding();
	}

	private rowAt(offset: number, from: HTMLElement): void {
		const rows = [...this.list.children] as HTMLElement[];
		const target = rows[rows.indexOf(from) + offset];
		if (target) target.focus();
		else if (offset < 0) this.heading.focus();
		else this.add.focus();
	}

	private row(property: Property, kind: PropertyKind, signature: string): Row {
		const { view } = this;
		const { key } = property;
		const row = element('div', 'cm-lp-property');
		row.tabIndex = 0;
		row.dataset.key = key;
		const keyEl = element('div', 'cm-lp-property-key');
		const icon = element('span', 'cm-lp-property-icon');
		icon.appendChild(strokeIcon(ICONS[kind], 'cm-lp-property-icon-svg'));
		const menu = (event: MouseEvent) => openPropertyMenu(view, event, property, kind);
		icon.addEventListener('mousedown', (event) => event.preventDefault());
		icon.addEventListener('click', menu);
		icon.addEventListener('contextmenu', menu);
		const keyInput = element('input', 'cm-lp-property-key-input');
		keyInput.value = key;
		keyInput.spellcheck = false;
		keyInput.autocapitalize = 'none';
		keyEl.append(icon, keyInput);
		const valueEl = element('div', 'cm-lp-property-value');
		valueEl.dataset.kind = kind;
		let editor: ValueEditor | null = null;
		if (kind === 'raw') {
			const raw = element('span', 'cm-lp-property-raw');
			raw.textContent = (property.raw ?? '').replace(/^[^\n]*?:[ \t]*/, '').trim();
			valueEl.appendChild(raw);
		} else {
			editor = valueEditor({
				view,
				key,
				kind,
				value: property.value,
				commit: (value, target) => setPropertyValue(view, key, value, target),
				leave: () => row.focus()
			});
			valueEl.appendChild(editor.el);
		}
		row.append(keyEl, valueEl);
		const rename = (focusValue: boolean) => {
			const next = keyInput.value.trim();
			if (next === key) {
				if (focusValue) editor?.focus('all');
				return;
			}
			if (checkKey(view, next, key)) renameProperty(view, key, next);
			else keyInput.value = key;
		};
		keyInput.addEventListener('keydown', (event) => {
			if (event.isComposing) return;
			if (event.key === 'Enter' || event.key === 'Tab') {
				event.preventDefault();
				rename(true);
			} else if (event.key === 'Escape') {
				event.preventDefault();
				keyInput.value = key;
				row.focus();
			}
		});
		keyInput.addEventListener('blur', () => {
			if (keyInput.value.trim() !== key) rename(false);
		});
		row.addEventListener('keydown', (event) => {
			if (event.target !== row || event.isComposing) return;
			const mod = event.metaKey || event.ctrlKey;
			const action = this.rowAction(event, mod, row, keyInput, editor, key);
			if (!action) return;
			event.preventDefault();
			action();
		});
		return { row, keyInput, editor, signature };
	}

	private rowAction(
		event: KeyboardEvent,
		mod: boolean,
		row: HTMLElement,
		keyInput: HTMLInputElement,
		editor: ValueEditor | null,
		key: string
	): (() => void) | null {
		const { view } = this;
		if (mod && event.key.toLowerCase() === 'z') return () => (event.shiftKey ? redo : undo)(view);
		if (event.key === 'Delete' || (mod && event.key === 'Backspace'))
			return () => removeProperties(view, [key], true);
		if (event.key === 'Tab') return () => this.rowAt(event.shiftKey ? -1 : 1, row);
		if (event.key === 'Home' || (event.altKey && event.key === 'ArrowDown'))
			return () => focusBody(view);
		if (mod || event.altKey) return null;
		const keys: Record<string, () => void> = {
			Enter: () => editor?.focus('all'),
			' ': () => editor?.focus('all'),
			A: () => editor?.focus('end'),
			i: () => editor?.focus('start'),
			ArrowLeft: () => keyInput.focus(),
			h: () => keyInput.focus(),
			ArrowRight: () => editor?.focus('all'),
			l: () => editor?.focus('all'),
			ArrowUp: () => this.rowAt(-1, row),
			k: () => this.rowAt(-1, row),
			ArrowDown: () => this.rowAt(1, row),
			j: () => this.rowAt(1, row),
			o: () => this.startAdding()
		};
		return keys[event.key] ?? null;
	}

	private startAdding(): void {
		if (this.collapsed) this.fold(false);
		if (this.pending) {
			this.pending.querySelector('input')?.focus();
			return;
		}
		const { view } = this;
		const row = element('div', 'cm-lp-property cm-lp-property-new');
		const keyEl = element('div', 'cm-lp-property-key');
		const icon = element('span', 'cm-lp-property-icon');
		icon.appendChild(strokeIcon(ICONS.text, 'cm-lp-property-icon-svg'));
		const input = element('input', 'cm-lp-property-key-input');
		input.spellcheck = false;
		input.autocapitalize = 'none';
		keyEl.append(icon, input);
		row.append(keyEl, element('div', 'cm-lp-property-value'));
		const dismiss = (refocus: boolean) => {
			if (this.pending !== row) return;
			this.pending = null;
			const previous = row.previousElementSibling as HTMLElement | null;
			row.remove();
			if (!this.rows.size) view.dispatch({ effects: setPropertiesUi.of({ adding: false }) });
			if (refocus) (previous ?? this.add).focus();
		};
		const commit = (refocus: boolean) => {
			const key = input.value.trim();
			if (!checkKey(view, key, null)) return false;
			this.pending = null;
			addProperty(view, key, refocus);
			return true;
		};
		input.addEventListener('keydown', (event) => {
			if (event.isComposing) return;
			if (event.key === 'Enter' || event.key === 'Tab') {
				event.preventDefault();
				commit(true);
			} else if (event.key === 'Escape') {
				event.preventDefault();
				dismiss(true);
			}
		});
		input.addEventListener('blur', () => {
			if (this.pending !== row) return;
			if (!input.value.trim() || !commit(false)) dismiss(false);
		});
		this.pending = row;
		this.list.appendChild(row);
		input.focus();
	}

	private fold(collapsed: boolean): void {
		this.view.dispatch({ effects: setPropertiesUi.of({ collapsed }) });
		this.heading.focus();
	}

	private headingKey(event: KeyboardEvent): void {
		if (event.isComposing || event.shiftKey || event.altKey || event.ctrlKey || event.metaKey)
			return;
		const first = this.list.firstElementChild as HTMLElement | null;
		const keys: Record<string, () => void> = {
			ArrowLeft: () => this.fold(true),
			ArrowRight: () => this.fold(false),
			ArrowUp: () => contextOf(this.view.state).focusTitle(),
			ArrowDown: () => (this.collapsed ? focusBody(this.view) : (first ?? this.add).focus()),
			j: () => (this.collapsed ? focusBody(this.view) : (first ?? this.add).focus())
		};
		const action = keys[event.key];
		if (!action) return;
		event.preventDefault();
		action();
	}

	private addKey(event: KeyboardEvent): void {
		if (event.isComposing) return;
		const last = this.list.lastElementChild as HTMLElement | null;
		const up =
			event.key === 'ArrowUp' || event.key === 'k' || (event.key === 'Tab' && event.shiftKey);
		const down = event.key === 'ArrowDown' || event.key === 'j' || event.key === 'Tab';
		if (event.key === 'Enter' || event.key === ' ') this.startAdding();
		else if (up) (last ?? this.heading).focus();
		else if (down) focusBody(this.view);
		else return;
		event.preventDefault();
	}

	private headingMenu(event: MouseEvent): void {
		openPropertiesMenu(this.view, event, [...this.rows.keys()], () => this.startAdding());
	}

	focus(target: FocusTarget): void {
		if (!('key' in target)) {
			if (target.part === 'heading') this.heading.focus();
			else if (target.part === 'add') (this.collapsed ? this.heading : this.add).focus();
			else if (target.part === 'body') focusBody(this.view);
			else this.startAdding();
			return;
		}
		const row = this.rows.get(target.key);
		if (!row) return;
		if (target.part === 'row') row.row.focus();
		else if (target.part === 'key') row.keyInput.focus();
		else if (target.part === 'pill' && row.editor?.focusPill) row.editor.focusPill(target.index);
		else row.editor?.focus('end');
	}
}

const panels = new WeakMap<HTMLElement, Panel>();

export class PropertiesWidget extends WidgetType {
	private readonly signature: string;

	constructor(
		readonly entries: readonly PanelEntry[],
		readonly collapsed: boolean,
		readonly adding: boolean
	) {
		super();
		this.signature = JSON.stringify([
			entries.map(({ property, kind }) => [property.key, kind, property.value, property.raw]),
			collapsed,
			adding
		]);
	}

	eq(other: PropertiesWidget): boolean {
		return other.signature === this.signature;
	}

	toDOM(view: EditorView): HTMLElement {
		const panel = new Panel(view);
		panels.set(panel.root, panel);
		panel.sync(this.entries, this.collapsed, this.adding);
		return panel.root;
	}

	updateDOM(dom: HTMLElement): boolean {
		const panel = panels.get(dom);
		if (!panel) return false;
		panel.sync(this.entries, this.collapsed, this.adding);
		return true;
	}

	ignoreEvent(): boolean {
		return true;
	}
}
