import type { Editor } from '@tiptap/core';
import { splitFrontmatter, joinFrontmatter } from '$lib/editor/frontmatter';
import { resolveImagePaths, resolveWikiEmbeds, unresolveImagePaths } from '$lib/editor/image-paths';

export interface NoteDocumentHost {
	folder(): string;
	vaultPath(): string | null;
}

export class NoteDocument {
	private readonly host: NoteDocumentHost;
	private frontmatter: string | null = null;

	constructor(host: NoteDocumentHost) {
		this.host = host;
	}

	toEditor(markdown: string): string {
		const split = splitFrontmatter(markdown);
		this.frontmatter = split.frontmatter;
		return resolveImagePaths(
			resolveWikiEmbeds(split.body, this.host.folder()),
			this.host.vaultPath()
		);
	}

	serialize(editor: Editor): string {
		const markdown = editor.storage.markdown?.getMarkdown?.() ?? '';
		return joinFrontmatter(this.frontmatter, unresolveImagePaths(markdown, this.host.vaultPath()));
	}
}
