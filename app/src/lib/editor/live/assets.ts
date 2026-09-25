import { syntaxTree } from '@codemirror/language';
import type { SyntaxNode } from '@lezer/common';
import type { EditorState, Line } from '@codemirror/state';
import { isImagePath } from '$lib/utils/mime';
import { contextOf } from './context';
import { sourcesOf } from './decorate';
import { embedExtension, resolveAsset, resolveEmbed, type ResolveSources } from './resolve';
import { EMBED } from './syntax';

export interface AssetNode {
	kind: 'image' | 'rule';
	from: number;
	to: number;
	line: Line;
	ownLine: boolean;
	alt: string;
	src: string | null;
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

function collectAssets(state: EditorState, sources: ResolveSources): AssetNode[] {
	const doc = state.doc;
	const out: AssetNode[] = [];
	syntaxTree(state).iterate({
		enter(ref) {
			if (ref.name === 'HorizontalRule') {
				const line = doc.lineAt(ref.from);
				out.push({
					kind: 'rule',
					from: ref.from,
					to: ref.to,
					line,
					ownLine: isOwnLine(state, ref.from, ref.to, line),
					alt: '',
					src: null
				});
				return;
			}
			if (ref.name !== 'Image' && ref.name !== EMBED) return;
			const text = doc.sliceString(ref.from, ref.to);
			const isEmbed = ref.name === EMBED;
			const target = isEmbed ? text.slice(3, -2) : imageTarget(state, ref.node, text);
			if (!target) return;
			if (isEmbed && !isImagePath(embedExtension(target))) return;
			const resolved = isEmbed ? resolveEmbed(target, sources) : resolveAsset(target, sources);
			if (!resolved) return;
			const line = doc.lineAt(ref.from);
			out.push({
				kind: 'image',
				from: ref.from,
				to: ref.to,
				line,
				ownLine: isOwnLine(state, ref.from, ref.to, line),
				alt: isEmbed ? target.split('|')[0] : text.slice(2, Math.max(text.indexOf(']'), 2)),
				src: resolved.abs
			});
		}
	});
	return out;
}

export function assetsOf(state: EditorState): AssetNode[] {
	return collectAssets(state, sourcesOf(contextOf(state)));
}
