import type { SyntaxNode } from '@lezer/common';
import type { EditorState, Line } from '@codemirror/state';
import { buildLocalfileUrl } from '$lib/editor/image-url';
import { isImagePath } from '$lib/utils/mime';
import { WEB_IMAGE } from '$lib/utils/web-link';
import { contextOf, staticPreview } from './context';
import { sourcesOf } from './decorate';
import { nodesNamed } from './doc-index';
import { embedExtension, resolveAsset, resolveEmbed, type ResolveSources } from './resolve';
import { EMBED } from './syntax';

export interface ImageRef {
	url: string;
	path: string | null;
	alt: string;
	width?: number;
	height?: number;
}

export interface AssetNode {
	kind: 'image' | 'rule';
	from: number;
	to: number;
	line: Line;
	ownLine: boolean;
	alt: string;
	url: string;
	path: string | null;
	width?: number;
	height?: number;
}

const SIZE = /^(\d+)(?:x(\d+))?$/;

function sized(alt: string, spec: string): Pick<ImageRef, 'alt' | 'width' | 'height'> | null {
	const size = SIZE.exec(spec.trim());
	if (!size) return null;
	return { alt, width: Number(size[1]), height: size[2] ? Number(size[2]) : undefined };
}

function located(
	target: string,
	embed: boolean,
	sources: ResolveSources
): Pick<ImageRef, 'url' | 'path'> | null {
	if (WEB_IMAGE.test(target.trim())) return { url: target.trim(), path: null };
	const resolved = embed ? resolveEmbed(target, sources) : resolveAsset(target, sources);
	return resolved && { url: buildLocalfileUrl(resolved.abs), path: resolved.abs };
}

// Inside a table the size separator is written `\|`, because a bare pipe would split the cell.
export function resolveImage(
	target: string,
	embed: boolean,
	label: string,
	state: EditorState
): ImageRef | null {
	const clean = target.replace(/\\\|/g, '|');
	const sources = sourcesOf(contextOf(state));
	let image: ImageRef;
	if (embed) {
		const [name, spec = ''] = clean.split('|');
		if (!isImagePath(embedExtension(name))) return null;
		const place = located(name, true, sources);
		if (!place) return null;
		image = { ...place, ...(sized(name, spec) ?? { alt: spec.trim() || name }) };
	} else {
		const place = located(clean, false, sources);
		if (!place) return null;
		const text = label.replace(/\\\|/g, '|');
		const cut = text.lastIndexOf('|');
		image = {
			...place,
			...((cut >= 0 && sized(text.slice(0, cut), text.slice(cut + 1))) || { alt: text })
		};
	}
	// Ask answers (and other static previews) must not fetch remote images: a note can inject a URL that leaks vault text.
	return image.path === null && state.facet(staticPreview) ? null : image;
}

function isOwnLine(state: EditorState, from: number, to: number, at: Line): boolean {
	return (
		!at.text.slice(0, Math.max(from - at.from, 0)).trim() && !at.text.slice(to - at.from).trim()
	);
}

function imageTarget(state: EditorState, node: SyntaxNode, text: string): string {
	const url = node.getChild('URL');
	if (url) return state.doc.sliceString(url.from, url.to);
	const open = text.indexOf('](');
	return open < 0 ? '' : text.slice(open + 2, text.endsWith(')') ? -1 : undefined);
}

function inTableWidget(node: SyntaxNode): boolean {
	for (let at = node.parent; at; at = at.parent)
		if (at.name === 'Table') return at.parent?.name === 'Document';
	return false;
}

export function assetsOf(state: EditorState, from = 0, to = state.doc.length): AssetNode[] {
	const doc = state.doc;
	const out: AssetNode[] = [];
	for (const ref of nodesNamed(state, 'HorizontalRule')) {
		if (ref.to < from || ref.from > to) continue;
		const line = doc.lineAt(ref.from);
		out.push({
			kind: 'rule',
			from: ref.from,
			to: ref.to,
			line,
			ownLine: isOwnLine(state, ref.from, ref.to, line),
			alt: '',
			url: '',
			path: null
		});
	}
	for (const ref of [...nodesNamed(state, 'Image'), ...nodesNamed(state, EMBED)]) {
		if (ref.to < from || ref.from > to || inTableWidget(ref)) continue;
		const text = doc.sliceString(ref.from, ref.to);
		const isEmbed = ref.name === EMBED;
		const target = isEmbed ? text.slice(3, -2) : imageTarget(state, ref, text);
		if (!target) continue;
		const label = isEmbed ? '' : text.slice(2, Math.max(text.indexOf(']'), 2));
		const image = resolveImage(target, isEmbed, label, state);
		if (!image) continue;
		const line = doc.lineAt(ref.from);
		out.push({
			kind: 'image',
			from: ref.from,
			to: ref.to,
			line,
			ownLine: isOwnLine(state, ref.from, ref.to, line),
			...image
		});
	}
	return out;
}
