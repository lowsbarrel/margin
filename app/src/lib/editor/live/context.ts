import { Facet, type EditorState } from '@codemirror/state';
import type { ContextMenuItem } from '$lib/components/ContextMenu.svelte';
import type { Highlighter } from './code-highlight';

export interface LiveContext {
	vaultPath(): string | null;
	notePath(): string;
	attachmentFolder(): string;
	exists(relPath: string): boolean;
	findByName(name: string): string | null;
	openLightbox(src: string, alt: string): void;
	openWikiLink(title: string): void;
	openContextMenu(x: number, y: number, items: ContextMenuItem[]): void;
	code: Highlighter | null;
}

export const liveContext = Facet.define<LiveContext, LiveContext>({
	combine: (values) => values[0]
});

// Read-only views (embed cards, Ask answers) have an idle selection at 0; they never reveal syntax.
export const staticPreview = Facet.define<boolean, boolean>({
	combine: (values) => values.some(Boolean)
});

export function contextOf(state: EditorState): LiveContext {
	return state.facet(liveContext);
}
