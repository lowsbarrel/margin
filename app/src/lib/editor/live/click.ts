import { syntaxTree } from '@codemirror/language';
import type { SyntaxNode } from '@lezer/common';
import { ViewPlugin, type EditorView } from '@codemirror/view';
import { openPath, openUrl } from '@tauri-apps/plugin-opener';
import { isLocalfileUrl, stripLocalfilePrefix, toOsPath } from '$lib/editor/image-url';
import { webHref } from '$lib/utils/web-link';
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

export type { LinkTarget };

export function targetAt(view: EditorView, pos: number): LinkTarget | null {
	let node: SyntaxNode | null = syntaxTree(view.state).resolveInner(pos, 1);
	while (node) {
		if (node.name === WIKI_LINK) {
			const text = view.state.doc.sliceString(node.from, node.to);
			const target = text
				.slice(2, Math.max(text.length - 2, 2))
				.split('|')[0]
				.trim();
			return target ? { kind: 'wiki', value: target } : null;
		}
		if (node.name === 'Link') {
			const url = node.getChild('URL');
			if (!url) return null;
			const href = view.state.doc.sliceString(url.from, url.to).replace(/^<|>$/g, '');
			return href ? { kind: 'url', value: href } : null;
		}
		if (node.name === 'Autolink' || (node.name === 'URL' && node.parent?.name !== 'Image')) {
			const url = node.name === 'URL' ? node : node.getChild('URL');
			return url
				? { kind: 'url', value: webHref(view.state.doc.sliceString(url.from, url.to)) }
				: null;
		}
		node = node.parent;
	}
	return null;
}

export function openHref(href: string, vaultPath: string | null): void {
	if (/^(https?|mailto):/.test(href)) {
		openUrl(href).catch((err) => toast.error(m.toast_cannot_open_url({ error: String(err) })));
		return;
	}
	if (!vaultPath || href.startsWith('#')) return;
	const abs = isLocalfileUrl(href)
		? toOsPath(decodeDestination(stripLocalfilePrefix(href) ?? ''))
		: toOsPath(`${vaultPath}/${decodeDestination(href)}`);
	openPath(abs).catch((err) => toast.error(m.toast_cannot_open_file({ error: String(err) })));
}

function linkUnder(event: MouseEvent, view: EditorView): LinkTarget | null {
	const anchor = (event.target as HTMLElement | null)?.closest('.cm-lp-link, .cm-lp-wikilink');
	return anchor ? targetAt(view, view.posAtDOM(anchor)) : null;
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

	const tag = target.closest<HTMLElement>('.cm-lp-tag:not([data-tag])');
	if (!tag || tag.closest('.cm-editor') !== view.dom) return false;
	event.preventDefault();
	ctx.openTag((tag.textContent ?? '').replace(/^#/, ''));
	return true;
}

function openLink(view: EditorView, link: LinkTarget): void {
	const ctx = contextOf(view.state);
	if (link.kind === 'wiki') ctx.openWikiLink(link.value);
	else openHref(link.value, ctx.vaultPath());
}

// A link opens on release: the window-level release re-renders revealed syntax, and WebKit drops the click whose target it detached.
export const liveClicks = ViewPlugin.fromClass(
	class {
		pressed: { link: LinkTarget; x: number; y: number } | null = null;

		constructor(readonly view: EditorView) {}
	},
	{
		eventHandlers: {
			mousedown(event) {
				const link = event.button === 0 ? linkUnder(event, this.view) : null;
				this.pressed = link ? { link, x: event.clientX, y: event.clientY } : null;
				return false;
			},
			mouseup(event, view) {
				const pressed = this.pressed;
				this.pressed = null;
				if (!pressed || event.button !== 0) return false;
				if (Math.abs(pressed.x - event.clientX) > 3 || Math.abs(pressed.y - event.clientY) > 3)
					return false;
				openLink(view, pressed.link);
				return true;
			},
			click(event, view) {
				return handleClick(event, view);
			}
		}
	}
);
