import type { TrashItem } from '$lib/history/bridge';

const PURGE_DAYS = 30;
export const PURGE_HINT_DAYS = 7;

const DAY_MS = 86_400_000;

export type GroupLabel = 'today' | 'yesterday' | 'date' | 'unknown';

export interface TrashGroup {
	key: string;
	label: GroupLabel;
	day: number | null;
	items: TrashItem[];
}

export type TrashRow =
	| { kind: 'group'; key: string; label: GroupLabel; day: number | null }
	| { kind: 'item'; key: string; item: TrashItem; index: number };

type RelativeUnit = 'now' | 'minute' | 'hour' | 'day';

export interface RelativeTime {
	unit: RelativeUnit;
	count: number;
}

export function filterItems(items: TrashItem[], query: string): TrashItem[] {
	const terms = query.trim().toLowerCase().split(/\s+/).filter(Boolean);
	if (terms.length === 0) return items;
	return items.filter((item) => {
		const haystack = `${item.name}\n${item.path}`.toLowerCase();
		return terms.every((term) => haystack.includes(term));
	});
}

function dayStart(ms: number): number {
	const date = new Date(ms);
	date.setHours(0, 0, 0, 0);
	return date.getTime();
}

export function groupItems(items: TrashItem[], now: number): TrashGroup[] {
	const today = dayStart(now);
	const previous = new Date(now);
	previous.setDate(previous.getDate() - 1);
	const yesterday = dayStart(previous.getTime());

	const groups: TrashGroup[] = [];
	for (const item of items) {
		const day = item.deleted_at == null ? null : dayStart(item.deleted_at);
		const last = groups[groups.length - 1];
		if (last && last.day === day) {
			last.items.push(item);
			continue;
		}
		const label: GroupLabel =
			day == null ? 'unknown' : day === today ? 'today' : day === yesterday ? 'yesterday' : 'date';
		groups.push({ key: day == null ? 'unknown' : String(day), label, day, items: [item] });
	}
	return groups;
}

export function toRows(groups: TrashGroup[]): TrashRow[] {
	const rows: TrashRow[] = [];
	let index = 0;
	for (const group of groups) {
		rows.push({ kind: 'group', key: group.key, label: group.label, day: group.day });
		for (const item of group.items) {
			rows.push({ kind: 'item', key: item.id, item, index: index++ });
		}
	}
	return rows;
}

export function formatDay(day: number, locale: string, now: number): string {
	const date = new Date(day);
	const sameYear = date.getFullYear() === new Date(now).getFullYear();
	return date.toLocaleDateString(locale, {
		month: 'long',
		day: 'numeric',
		...(sameYear ? {} : { year: 'numeric' })
	});
}

export function relativeTime(deletedAt: number | null, now: number): RelativeTime | null {
	if (deletedAt == null) return null;
	const minutes = Math.floor((now - deletedAt) / 60_000);
	if (minutes < 1) return { unit: 'now', count: 0 };
	if (minutes < 60) return { unit: 'minute', count: minutes };
	const hours = Math.floor(minutes / 60);
	if (hours < 24) return { unit: 'hour', count: hours };
	return { unit: 'day', count: Math.floor(hours / 24) };
}

export function daysUntilPurge(deletedAt: number | null, now: number): number | null {
	if (deletedAt == null) return null;
	return Math.max(0, Math.floor((deletedAt + PURGE_DAYS * DAY_MS - now) / DAY_MS));
}
