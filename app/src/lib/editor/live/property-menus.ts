import type { EditorView } from '@codemirror/view';
import type { ContextMenuItem } from '$lib/components/ContextMenu.svelte';
import * as m from '$lib/paraglide/messages.js';
import { toast } from '$lib/stores/toast.svelte';
import { contextOf } from './context';
import type { Property, PropertyKind } from './frontmatter';
import {
	changeKind,
	convertValue,
	removeProperties,
	sortProperties,
	takenKey
} from './property-edits';

export const PROPERTY_ICONS: Record<PropertyKind | 'plus' | 'fold', string> = {
	text: '<path d="M17 6.1H3"/><path d="M21 12.1H3"/><path d="M15.1 18H3"/>',
	list: '<path d="M3 12h.01"/><path d="M3 18h.01"/><path d="M3 6h.01"/><path d="M8 12h13"/><path d="M8 18h13"/><path d="M8 6h13"/>',
	number:
		'<rect x="14" y="14" width="4" height="6" rx="2"/><rect x="6" y="4" width="4" height="6" rx="2"/><path d="M6 20h4"/><path d="M14 10h4"/><path d="M6 14h2v6"/><path d="M14 4h2v6"/>',
	checkbox: '<rect width="18" height="18" x="3" y="3" rx="2"/><path d="m9 12 2 2 4-4"/>',
	date: '<path d="M8 2v4"/><path d="M16 2v4"/><rect width="18" height="18" x="3" y="4" rx="2"/><path d="M3 10h18"/>',
	datetime: '<circle cx="12" cy="12" r="10"/><polyline points="12 6 12 12 16 14"/>',
	tags: '<path d="m15 5 6.3 6.3a2.4 2.4 0 0 1 0 3.4L17 19"/><path d="M9.59 5.59A2 2 0 0 0 8.17 5H3a1 1 0 0 0-1 1v5.17a2 2 0 0 0 .59 1.42l5.7 5.7a2.43 2.43 0 0 0 3.42 0l3.58-3.58a2.43 2.43 0 0 0 0-3.42z"/><circle cx="6.5" cy="9.5" r=".5"/>',
	aliases: '<polyline points="15 17 20 12 15 7"/><path d="M4 18v-2a4 4 0 0 1 4-4h12"/>',
	raw: '<circle cx="12" cy="12" r="10"/><path d="M12 16v-4"/><path d="M12 8h.01"/>',
	plus: '<path d="M5 12h14"/><path d="M12 5v14"/>',
	fold: '<path d="m6 9 6 6 6-6"/>'
};

const TYPES: [PropertyKind, () => string][] = [
	['text', m.editor_property_type_text],
	['list', m.editor_property_type_list],
	['number', m.editor_property_type_number],
	['checkbox', m.editor_property_type_checkbox],
	['date', m.editor_property_type_date],
	['datetime', m.editor_property_type_datetime]
];

export function checkKey(view: EditorView, key: string, current: string | null): boolean {
	if (!key) toast.error(m.editor_property_name_empty());
	else if (takenKey(view, key, current)) toast.error(m.editor_property_name_taken());
	else return true;
	return false;
}

function open(view: EditorView, event: MouseEvent, items: ContextMenuItem[]): void {
	event.preventDefault();
	event.stopPropagation();
	contextOf(view.state).openContextMenu(event.clientX, event.clientY, items);
}

export function openPropertiesMenu(
	view: EditorView,
	event: MouseEvent,
	keys: readonly string[],
	startAdding: () => void
): void {
	open(view, event, [
		{ label: m.editor_property_add(), onclick: startAdding },
		{ label: m.editor_properties_sort_ascending(), onclick: () => sortProperties(view, false) },
		{ label: m.editor_properties_sort_descending(), onclick: () => sortProperties(view, true) },
		{
			label: m.editor_properties_clear(),
			destructive: true,
			onclick: () => removeProperties(view, keys, false)
		}
	]);
}

export function openPropertyMenu(
	view: EditorView,
	event: MouseEvent,
	property: Property,
	kind: PropertyKind
): void {
	const fixed = kind === 'raw' || kind === 'tags' || kind === 'aliases';
	const types: ContextMenuItem[] = TYPES.map(([type, label]) => ({
		label: label(),
		checked: kind === type,
		disabled: fixed || convertValue(property.value, type) === undefined,
		onclick: () => changeKind(view, property.key, type)
	}));
	open(view, event, [
		...types,
		{
			label: m.editor_property_remove(),
			destructive: true,
			onclick: () => removeProperties(view, [property.key], true)
		}
	]);
}
