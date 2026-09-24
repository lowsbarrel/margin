import { selectAll } from '@codemirror/commands';
import { Prec } from '@codemirror/state';
import { ViewPlugin, type EditorView } from '@codemirror/view';
import { openPath, revealItemInDir } from '@tauri-apps/plugin-opener';
import type { ContextMenuItem } from '$lib/components/ContextMenu.svelte';
import { toast } from '$lib/stores/toast.svelte';
import { panes } from '$lib/stores/panes.svelte';
import * as m from '$lib/paraglide/messages.js';
import { assetsOf } from './assets';
import { openHref, resolveAbsPath, targetAt, type LinkTarget } from './click';
import type { LiveContext } from './context';
import { contextOf } from './context';
import { pasteFromClipboard } from './paste';
import {
	alignTableColumn,
	deleteTableColumn,
	deleteTableRow,
	insertTableColumn,
	insertTableRow,
	toggleList,
	toggleQuote
} from './commands';
import { toggleMark } from './marks';
import { locateCell, tableAt } from './table-model';

const FORMATS: { label: () => string; run: (view: EditorView) => boolean }[] = [
	{ label: () => m.bubble_bold(), run: (view) => toggleMark(view, 'bold') },
	{ label: () => m.bubble_italic(), run: (view) => toggleMark(view, 'italic') },
	{ label: () => m.bubble_strike(), run: (view) => toggleMark(view, 'strike') },
	{ label: () => m.bubble_highlight(), run: (view) => toggleMark(view, 'highlight') },
	{ label: () => m.bubble_code(), run: (view) => toggleMark(view, 'code') },
	{ label: () => m.bubble_block_quote(), run: (view) => toggleQuote(view) },
	{ label: () => m.bubble_block_bullet(), run: (view) => toggleList(view, 'bullet') },
	{ label: () => m.bubble_block_task(), run: (view) => toggleList(view, 'task') }
];

async function writeClipboard(text: string): Promise<void> {
	try {
		await navigator.clipboard.writeText(text);
	} catch (err) {
		toast.error(m.toast_clipboard_failed({ error: String(err) }));
	}
}

function copySelection(view: EditorView, cut: boolean): void {
	const { from, to } = view.state.selection.main;
	if (from === to) return;
	const text = view.state.sliceDoc(from, to);
	void navigator.clipboard
		.writeText(text)
		.then(() => {
			if (cut) {
				view.dispatch({ changes: { from, to }, userEvent: 'delete.cut', scrollIntoView: true });
			}
		})
		.catch((err) => toast.error(m.toast_clipboard_failed({ error: String(err) })));
}

function imageItems(
	image: HTMLImageElement,
	view: EditorView,
	ctx: LiveContext
): ContextMenuItem[] {
	const items: ContextMenuItem[] = [
		{
			label: m.editor_view_image(),
			onclick: () => ctx.openLightbox(image.src, image.alt || 'Image')
		}
	];
	const pos = view.posAtDOM(image);
	const asset = assetsOf(view.state).find(
		(entry) => entry.kind === 'image' && entry.line.from <= pos && pos <= entry.line.to
	);
	const abs = asset?.src ?? resolveAbsPath(image.src, ctx.vaultPath());
	if (!abs) return items;
	return [
		...items,
		{
			label: m.editor_open_default_app(),
			onclick: () =>
				openPath(abs).catch((err) => toast.error(m.toast_cannot_open_file({ error: String(err) })))
		},
		{
			label: m.editor_copy_path(),
			onclick: () => writeClipboard(abs)
		},
		{
			label: m.editor_reveal_in_finder(),
			onclick: () =>
				revealItemInDir(abs).catch((err) =>
					toast.error(m.toast_cannot_reveal_file({ error: String(err) }))
				)
		}
	];
}

function linkItems(target: LinkTarget, ctx: LiveContext): ContextMenuItem[] {
	if (target.kind !== 'wiki') {
		return [
			{ label: m.editor_open_link(), onclick: () => openHref(target.value, ctx.vaultPath()) },
			{ label: m.editor_copy_link(), onclick: () => writeClipboard(target.value) }
		];
	}
	const items: ContextMenuItem[] = [
		{ label: m.editor_open_link(), onclick: () => ctx.openWikiLink(target.value) }
	];
	const vaultPath = ctx.vaultPath();
	const rel = vaultPath
		? (ctx.findByName(`${target.value}.md`) ?? ctx.findByName(target.value))
		: null;
	if (rel && vaultPath) {
		items.push({
			label: m.editor_open_new_pane(),
			onclick: () => panes.openFileInNewPane(`${vaultPath}/${rel}`, panes.activePaneIndex, 'right')
		});
	}
	return items;
}

function tableItems(view: EditorView, event: MouseEvent, pos: number | null): ContextMenuItem[] {
	if (pos == null) return [];
	const table = tableAt(view.state, pos);
	if (!table) return [];
	const cell = (event.target as HTMLElement | null)?.closest(
		'.cm-lp-table-cell'
	) as HTMLElement | null;
	const tr = cell?.parentElement as HTMLElement | null;
	const fromWidget =
		cell && tr
			? {
					row: Array.prototype.indexOf.call(tr.parentElement?.children ?? [], tr),
					col: Array.prototype.indexOf.call(tr.children, cell)
				}
			: locateCell(table, pos);
	const row = fromWidget?.row ?? 0;
	const col = fromWidget?.col ?? 0;
	const at = table.from;
	return [
		{
			label: m.editor_table_insert_row_above(),
			onclick: () => void insertTableRow(view, row, 'above', at)
		},
		{
			label: m.editor_table_insert_row_below(),
			onclick: () => void insertTableRow(view, row, 'below', at)
		},
		{
			label: m.editor_table_insert_column_left(),
			onclick: () => void insertTableColumn(view, col, 'left', at)
		},
		{
			label: m.editor_table_insert_column_right(),
			onclick: () => void insertTableColumn(view, col, 'right', at)
		},
		{
			label: m.editor_table_delete_row(),
			destructive: true,
			onclick: () => void deleteTableRow(view, row, at)
		},
		{
			label: m.editor_table_delete_column(),
			destructive: true,
			onclick: () => void deleteTableColumn(view, col, at)
		},
		{
			label: m.editor_table_align_left(),
			onclick: () => void alignTableColumn(view, col, 'left', at)
		},
		{
			label: m.editor_table_align_center(),
			onclick: () => void alignTableColumn(view, col, 'center', at)
		},
		{
			label: m.editor_table_align_right(),
			onclick: () => void alignTableColumn(view, col, 'right', at)
		}
	];
}

function buildItems(view: EditorView, event: MouseEvent): ContextMenuItem[] {
	const ctx = contextOf(view.state);
	const items: ContextMenuItem[] = [];
	const image = (event.target as HTMLElement | null)?.closest(
		'.cm-lp-image'
	) as HTMLImageElement | null;
	const pos = view.posAtCoords({ x: event.clientX, y: event.clientY });
	if (image) items.push(...imageItems(image, view, ctx));
	else {
		items.push(...tableItems(view, event, pos));
		const target = pos == null ? null : targetAt(view, pos);
		if (target) items.push(...linkItems(target, ctx));
		if (!view.state.selection.main.empty) {
			items.push(
				...FORMATS.map((format) => ({
					label: format.label(),
					onclick: () => void format.run(view)
				}))
			);
		}
	}
	const hasSelection = !view.state.selection.main.empty;
	return [
		...items,
		{ label: m.editor_cut(), disabled: !hasSelection, onclick: () => copySelection(view, true) },
		{ label: m.editor_copy(), disabled: !hasSelection, onclick: () => copySelection(view, false) },
		{ label: m.editor_paste(), onclick: () => pasteFromClipboard(view, false) },
		{ label: m.editor_paste_plain(), onclick: () => pasteFromClipboard(view, true) },
		{ label: m.editor_select_all(), onclick: () => void selectAll(view) }
	];
}

export const liveContextMenu = Prec.high(
	ViewPlugin.fromClass(
		class {
			constructor(readonly view: EditorView) {}
		},
		{
			eventHandlers: {
				contextmenu(event, view) {
					const mouse = event as MouseEvent;
					mouse.preventDefault();
					const pos = view.posAtCoords({ x: mouse.clientX, y: mouse.clientY });
					const range = view.state.selection.main;
					if (pos != null && (pos < range.from || pos > range.to)) {
						view.dispatch({ selection: { anchor: pos } });
					}
					contextOf(view.state).openContextMenu(
						mouse.clientX,
						mouse.clientY,
						buildItems(view, mouse)
					);
					return true;
				}
			}
		}
	)
);
