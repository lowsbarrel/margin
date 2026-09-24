import { syntaxTree } from '@codemirror/language';
import type { SyntaxNode } from '@lezer/common';
import { ViewPlugin, type EditorView } from '@codemirror/view';
import { openPath, openUrl, revealItemInDir } from '@tauri-apps/plugin-opener';
import { isLocalfileUrl, stripLocalfilePrefix, toOsPath } from '$lib/editor/image-url';
import type { ContextMenuItem } from '$lib/components/ContextMenu.svelte';
import { toast } from '$lib/stores/toast.svelte';
import * as m from '$lib/paraglide/messages.js';
import { contextOf } from './context';
import { decodeDestination } from './resolve';
import { WIKI_LINK } from './syntax';

interface LinkTarget {
	kind: 'wiki' | 'url';
	value: string;
}

export function resolveAbsPath(src: string, vaultPath: string | null): string | null {
	if (isLocalfileUrl(src)) {
		const tail = stripLocalfilePrefix(src) ?? '';
		return toOsPath(decodeURIComponent(tail));
	}
	if (!/^(https?:|data:|blob:)/.test(src) && vaultPath) return toOsPath(`${vaultPath}/${src}`);
	return null;
}

function targetAt(view: EditorView, pos: number): LinkTarget | null {
	let node: SyntaxNode | null = syntaxTree(view.state).resolveInner(pos, 1);
	while (node) {
		if (node.name === WIKI_LINK) {
			const text = view.state.doc.sliceString(node.from, node.to);
			const title = text
				.slice(2, Math.max(text.length - 2, 2))
				.split('|')[0]
				.split('#')[0]
				.trim();
			return title ? { kind: 'wiki', value: title } : null;
		}
		if (node.name === 'Link' || node.name === 'Autolink') {
			const url = node.getChild('URL');
			if (!url) return null;
			const href = view.state.doc.sliceString(url.from, url.to).replace(/^<|>$/g, '');
			return href ? { kind: 'url', value: href } : null;
		}
		node = node.parent;
	}
	return null;
}

function openHref(href: string, vaultPath: string | null): void {
	if (/^https?:/.test(href)) {
		openUrl(href).catch((err) => toast.error(m.toast_cannot_open_url({ error: String(err) })));
		return;
	}
	if (!vaultPath || href.startsWith('#')) return;
	const abs = isLocalfileUrl(href)
		? toOsPath(decodeDestination(stripLocalfilePrefix(href) ?? ''))
		: toOsPath(`${vaultPath}/${decodeDestination(href)}`);
	openPath(abs).catch((err) => toast.error(m.toast_cannot_open_file({ error: String(err) })));
}

function imageMenu(image: HTMLImageElement, view: EditorView, x: number, y: number) {
	const ctx = contextOf(view.state);
	const items: ContextMenuItem[] = [
		{
			label: m.editor_view_image(),
			onclick: () => ctx.openLightbox(image.src, image.alt || 'Image')
		}
	];
	const abs = resolveAbsPath(image.src, ctx.vaultPath());
	if (abs) {
		items.push(
			{
				label: m.editor_open_default_app(),
				onclick: () =>
					openPath(abs).catch((err) =>
						toast.error(m.toast_cannot_open_file({ error: String(err) }))
					)
			},
			{
				label: m.editor_reveal_in_finder(),
				onclick: () =>
					revealItemInDir(abs).catch((err) =>
						toast.error(m.toast_cannot_reveal_file({ error: String(err) }))
					)
			}
		);
	}
	ctx.openContextMenu(x, y, items);
}

function handleClick(event: MouseEvent, view: EditorView): boolean {
	const target = event.target as HTMLElement | null;
	if (!target) return false;
	const ctx = contextOf(view.state);

	const image = target.closest('.cm-lp-image') as HTMLImageElement | null;
	if (image) {
		event.preventDefault();
		ctx.openLightbox(image.src, image.alt || 'Image');
		return true;
	}

	const anchor = target.closest('.cm-lp-link, .cm-lp-wikilink');
	if (!anchor) return false;
	const found = targetAt(view, view.posAtDOM(anchor));
	if (!found) return false;
	event.preventDefault();
	if (found.kind === 'wiki') ctx.openWikiLink(found.value);
	else openHref(found.value, ctx.vaultPath());
	return true;
}

export const liveClicks = ViewPlugin.fromClass(
	class {
		constructor(readonly view: EditorView) {}
	},
	{
		eventHandlers: {
			click(event, view) {
				return handleClick(event as MouseEvent, view);
			},
			contextmenu(event, view) {
				const mouse = event as MouseEvent;
				const image = (mouse.target as HTMLElement | null)?.closest(
					'.cm-lp-image'
				) as HTMLImageElement | null;
				if (!image) return false;
				mouse.preventDefault();
				imageMenu(image, view, mouse.clientX, mouse.clientY);
				return true;
			}
		}
	}
);
