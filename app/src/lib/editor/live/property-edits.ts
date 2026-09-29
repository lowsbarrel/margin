import { StateEffect, StateField, type ChangeSpec, type EditorState } from '@codemirror/state';
import type { EditorView } from '@codemirror/view';
import { frontmatterEnd } from './decorate';
import {
	asList,
	isListKind,
	kindOf,
	parseFrontmatter,
	renameSource,
	serializeProperty,
	type Property,
	type PropertyKind,
	type PropertyValue
} from './frontmatter';
import { focusEditor } from './table-cell';

export interface PropertiesUi {
	collapsed: boolean;
	adding: boolean;
	kinds: Readonly<Record<string, PropertyKind>>;
}

export type FocusTarget =
	| { part: 'heading' | 'add' | 'body' | 'new' }
	| { part: 'row' | 'key' | 'value'; key: string }
	| { part: 'pill'; key: string; index: number };

export const setPropertiesUi = StateEffect.define<Partial<PropertiesUi>>();

export const propertiesUi = StateField.define<PropertiesUi>({
	create: () => ({ collapsed: false, adding: false, kinds: {} }),
	update(value, tr) {
		let next = value;
		for (const effect of tr.effects)
			if (effect.is(setPropertiesUi)) next = { ...next, ...effect.value };
		return next;
	}
});

const focusers = new WeakMap<HTMLElement, (target: FocusTarget) => void>();

export function registerFocuser(panel: HTMLElement, focus: (target: FocusTarget) => void): void {
	focusers.set(panel, focus);
}

export function focusPanel(view: EditorView, target: FocusTarget): void {
	const panel = view.dom.querySelector<HTMLElement>('.cm-lp-properties');
	const focus = panel && focusers.get(panel);
	if (focus) focus(target);
}

export function propertyKind(state: EditorState, property: Property): PropertyKind {
	const inferred = kindOf(property.key, property.value, property.raw);
	const chosen = state.field(propertiesUi).kinds[property.key];
	if (!chosen || inferred === 'raw' || inferred === 'tags' || inferred === 'aliases')
		return inferred;
	const fits =
		JSON.stringify(convertValue(property.value, chosen)) === JSON.stringify(property.value);
	return fits ? chosen : inferred;
}

export function focusBody(view: EditorView): void {
	const end = frontmatterEnd(view.state);
	view.dispatch({ selection: { anchor: end ? Math.min(view.state.doc.length, end + 1) : 0 } });
	focusEditor(view);
}

function normalized(value: PropertyValue): PropertyValue {
	return value === '' || (Array.isArray(value) && value.length === 0) ? null : value;
}

function find(view: EditorView, key: string) {
	const frontmatter = parseFrontmatter(view.state);
	const property = frontmatter?.properties.find((entry) => entry.key === key) ?? null;
	return { frontmatter, property };
}

function apply(
	view: EditorView,
	changes: ChangeSpec,
	target: FocusTarget | null,
	ui?: Partial<PropertiesUi>
): void {
	view.dispatch({
		changes,
		effects: ui ? setPropertiesUi.of(ui) : [],
		userEvent: 'input.property'
	});
	if (target) focusPanel(view, target);
}

export function setPropertyValue(
	view: EditorView,
	key: string,
	value: PropertyValue,
	target: FocusTarget | null
): void {
	const { property } = find(view, key);
	if (!property) return;
	const next = normalized(value);
	if (property.raw === null && JSON.stringify(property.value) === JSON.stringify(next)) {
		if (target) focusPanel(view, target);
		return;
	}
	apply(
		view,
		{ from: property.from, to: property.to, insert: serializeProperty(key, next) },
		target
	);
}

export function takenKey(view: EditorView, key: string, except: string | null): boolean {
	const lower = key.toLowerCase();
	return !!parseFrontmatter(view.state)?.properties.some(
		(entry) => entry.key !== except && entry.key.toLowerCase() === lower
	);
}

export function renameProperty(view: EditorView, key: string, next: string): void {
	const { property } = find(view, key);
	if (!property || key === next) return;
	const insert =
		property.raw === null
			? serializeProperty(next, property.value)
			: renameSource(property.raw, next);
	const kinds = { ...view.state.field(propertiesUi).kinds };
	if (kinds[key]) kinds[next] = kinds[key];
	apply(
		view,
		{ from: property.from, to: property.to, insert },
		{ part: 'value', key: next },
		{ kinds }
	);
}

export function addProperty(view: EditorView, key: string, refocus: boolean): void {
	const frontmatter = parseFrontmatter(view.state);
	const line = serializeProperty(key, null);
	const changes = frontmatter
		? { from: frontmatter.close, insert: `${line}\n` }
		: { from: 0, insert: `---\n${line}\n---\n` };
	apply(view, changes, refocus ? { part: 'value', key } : null, { adding: false });
}

export function removeProperties(
	view: EditorView,
	keys: readonly string[],
	refocus: boolean
): void {
	const frontmatter = parseFrontmatter(view.state);
	if (!frontmatter) return;
	const doc = view.state.doc;
	const gone = frontmatter.properties.filter((entry) => keys.includes(entry.key));
	if (!gone.length) return;
	const left = frontmatter.properties.filter((entry) => !keys.includes(entry.key));
	if (!left.length) {
		const to = Math.min(doc.length, frontmatter.end + 1);
		apply(view, { from: 0, to }, refocus ? { part: 'body' } : null);
		return;
	}
	const index = frontmatter.properties.findIndex((entry) => entry.key === gone[0].key);
	const neighbour = left[Math.min(index, left.length - 1)];
	apply(
		view,
		gone.map((entry) => ({ from: entry.from, to: Math.min(doc.length, entry.to + 1) })),
		refocus ? { part: 'row', key: neighbour.key } : null
	);
}

export function sortProperties(view: EditorView, descending: boolean): void {
	const frontmatter = parseFrontmatter(view.state);
	const properties = frontmatter?.properties ?? [];
	if (properties.length < 2) return;
	const doc = view.state.doc;
	const sorted = [...properties].sort((a, b) =>
		descending ? b.key.localeCompare(a.key) : a.key.localeCompare(b.key)
	);
	const insert = sorted.map((entry) => doc.sliceString(entry.from, entry.to)).join('\n');
	apply(view, { from: properties[0].from, to: properties.at(-1)!.to, insert }, null);
}

export function convertValue(value: PropertyValue, kind: PropertyKind): PropertyValue | undefined {
	if (value === null) return null;
	if (isListKind(kind)) return asList(value);
	if (Array.isArray(value)) return kind === 'text' ? value.join(', ') : undefined;
	const text = String(value);
	if (kind === 'text') return text;
	if (kind === 'number')
		return text.trim() && Number.isFinite(Number(text)) ? Number(text) : undefined;
	if (kind === 'checkbox') return text === 'true' ? true : text === 'false' ? false : undefined;
	const date = /^(\d{4}-[01]\d-[0-3]\d)(?:T([0-2]\d:[0-5]\d(?::[0-5]\d)?))?$/.exec(text);
	if (!date) return undefined;
	return kind === 'date' ? date[1] : `${date[1]}T${date[2] ?? '00:00:00'}`;
}

export function changeKind(view: EditorView, key: string, kind: PropertyKind): void {
	const { property } = find(view, key);
	if (!property) return;
	const value = convertValue(property.value, kind);
	if (value === undefined) return;
	const kinds = { ...view.state.field(propertiesUi).kinds, [key]: kind };
	const changes =
		JSON.stringify(value) === JSON.stringify(property.value)
			? []
			: { from: property.from, to: property.to, insert: serializeProperty(key, normalized(value)) };
	apply(view, changes, { part: 'row', key }, { kinds });
}
