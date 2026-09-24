import { syntaxTree } from '@codemirror/language';
import { Facet, type EditorState, type Extension, type Line } from '@codemirror/state';
import { isImagePath } from '$lib/utils/mime';
import type { LiveContext } from './context';
import { normalizeRel, noteDir } from './resolve';
import { EMBED } from './syntax';

export interface EmbedTarget {
	from: number;
	to: number;
	line: Line;
	ownLine: boolean;
	kind: 'note' | 'file';
	name: string;
	heading: string | null;
	abs: string | null;
}

export type CardExtensions = (
	ctx: LiveContext,
	depth: number,
	chain: readonly string[]
) => Extension[];

export const MAX_EMBED_DEPTH = 2;

const SVG_NS = 'http://www.w3.org/2000/svg';
const ICON_PATHS: Record<'note' | 'file', string> = {
	note: '<path d="M15 2H6a2 2 0 0 0-2 2v16a2 2 0 0 0 2 2h12a2 2 0 0 0 2-2V7Z"/><path d="M14 2v4a2 2 0 0 0 2 2h4"/><path d="M10 13H8"/><path d="M16 13H14"/><path d="M10 17H8"/><path d="M16 17H14"/>',
	file: '<path d="m21.4 11.1-9.2 9.2a5 5 0 0 1-7-7l9.2-9.2a3.3 3.3 0 0 1 4.7 4.7l-9.2 9.2a1.7 1.7 0 0 1-2.3-2.3l8.5-8.5"/>'
};

export const embedDepth = Facet.define<number, number>({ combine: (values) => values[0] ?? 0 });
export const embedChain = Facet.define<readonly string[], readonly string[]>({
	combine: (values) => values[0] ?? []
});

function extensionOf(name: string): string {
	const dot = name.lastIndexOf('.');
	return dot <= 0 ? '' : name.slice(dot + 1).toLowerCase();
}

function headingSplit(raw: string): [string, string | null] {
	const hash = raw.indexOf('#');
	if (hash < 0) return [raw.trim(), null];
	return [raw.slice(0, hash).trim(), raw.slice(hash + 1).trim() || null];
}

function resolveTarget(ctx: LiveContext, kind: 'note' | 'file', written: string): string | null {
	const variants =
		kind === 'note' && !/\.md$/i.test(written) ? [`${written}.md`, written] : [written];
	if (written.includes('/')) {
		for (const variant of variants) {
			const rel = normalizeRel(variant);
			if (rel && !rel.startsWith('..') && ctx.exists(rel)) return rel;
		}
		return null;
	}
	const dir = noteDir(ctx.notePath(), ctx.vaultPath());
	const folder = ctx.attachmentFolder();
	for (const root of [dir, '', folder]) {
		for (const variant of variants) {
			const rel = normalizeRel(root ? `${root}/${variant}` : variant);
			if (rel && !rel.startsWith('..') && ctx.exists(rel)) return rel;
		}
	}
	return ctx.findByName(variants[0]) ?? null;
}

export function embedTargetsOf(state: EditorState, ctx: LiveContext): EmbedTarget[] {
	const doc = state.doc;
	const vault = ctx.vaultPath();
	const out: EmbedTarget[] = [];
	syntaxTree(state).iterate({
		enter(ref) {
			if (ref.name !== EMBED) return;
			const text = doc.sliceString(ref.from, ref.to);
			const inner = text.slice(3, Math.max(3, text.length - 2));
			const [rawName] = inner.split('|');
			const [written, heading] = headingSplit(rawName);
			if (!written || isImagePath(written)) return;
			const extension = extensionOf(written);
			const kind = extension === '' || extension === 'md' ? 'note' : 'file';
			const name = kind === 'note' ? written.replace(/\.md$/i, '') : written;
			const rel = resolveTarget(ctx, kind, written);
			const at = doc.lineAt(ref.from);
			out.push({
				from: ref.from,
				to: ref.to,
				line: at,
				ownLine:
					!at.text.slice(0, Math.max(ref.from - at.from, 0)).trim() &&
					!at.text.slice(ref.to - at.from).trim(),
				kind,
				name,
				heading,
				abs: rel && vault ? `${vault}/${rel}` : null
			});
		}
	});
	return out;
}

export function embedIcon(kind: 'note' | 'file'): SVGSVGElement {
	const svg = document.createElementNS(SVG_NS, 'svg');
	svg.setAttribute('viewBox', '0 0 24 24');
	svg.setAttribute('fill', 'none');
	svg.setAttribute('stroke', 'currentColor');
	svg.setAttribute('stroke-width', '1.7');
	svg.setAttribute('stroke-linecap', 'round');
	svg.setAttribute('stroke-linejoin', 'round');
	svg.setAttribute('aria-hidden', 'true');
	svg.classList.add('cm-lp-embed-icon');
	svg.innerHTML = ICON_PATHS[kind];
	return svg;
}
