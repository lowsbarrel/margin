import { EditorState, type Extension } from '@codemirror/state';
import { EditorView, ViewPlugin, WidgetType } from '@codemirror/view';
import { fileMetadata, readFileBytes } from '$lib/fs/bridge';
import { panes } from '$lib/stores/panes.svelte';
import * as m from '$lib/paraglide/messages.js';
import { blockWidgets } from './blocks';
import { liveCallouts } from './callouts';
import { liveClicks } from './click';
import { contextOf, liveContext, type LiveContext } from './context';
import {
	embedChain,
	embedDepth,
	embedIcon,
	MAX_EMBED_DEPTH,
	type CardExtensions,
	type EmbedTarget
} from './embed-targets';
import { liveFootnotes } from './footnotes';
import { liveMath } from './math';
import { liveMermaid } from './mermaid';
import { livePreview } from './preview';
import { markdownSyntax } from './syntax';

const ATX_HEADING = /^(#{1,6})[ \t]+(.*?)[ \t]*#*[ \t]*$/;

function messageElement(className: string, text: string): HTMLElement {
	const element = document.createElement('div');
	element.className = className;
	element.textContent = text;
	return element;
}

function formatSize(bytes: number): string {
	if (bytes < 1024) return `${bytes} B`;
	const kb = bytes / 1024;
	if (kb < 1024) return `${kb < 10 ? kb.toFixed(1) : Math.round(kb)} KB`;
	const mb = kb / 1024;
	return `${mb < 10 ? mb.toFixed(1) : Math.round(mb)} MB`;
}

function headingSection(text: string, heading: string): string {
	const lines = text.split('\n');
	const wanted = heading.trim().toLowerCase();
	let start = -1;
	let level = 0;
	for (let i = 0; i < lines.length; i++) {
		const match = ATX_HEADING.exec(lines[i]);
		if (!match) continue;
		if (start < 0) {
			if (match[2].trim().toLowerCase() === wanted) {
				start = i;
				level = match[1].length;
			}
			continue;
		}
		if (match[1].length <= level) return lines.slice(start, i).join('\n');
	}
	return start < 0 ? text : lines.slice(start).join('\n');
}

// A transclusion is read-only: a task box inside it must not dispatch into the embedded copy.
const readOnlyCards = ViewPlugin.fromClass(
	class {
		constructor(view: EditorView) {
			view.contentDOM.addEventListener(
				'click',
				(event) => {
					const target = event.target as HTMLElement | null;
					if (!target?.closest('.cm-lp-checkbox')) return;
					event.preventDefault();
					event.stopPropagation();
				},
				true
			);
		}
	}
);

export function cardExtensions(
	ctx: LiveContext,
	depth: number,
	chain: readonly string[],
	liveEmbeds: Extension
): Extension[] {
	return [
		liveContext.of(ctx),
		embedDepth.of(depth),
		embedChain.of(chain),
		markdownSyntax,
		EditorView.editorAttributes.of({ class: 'cm-lp-embed-editor' }),
		EditorView.editable.of(false),
		EditorView.lineWrapping,
		readOnlyCards,
		livePreview,
		blockWidgets,
		liveClicks,
		liveCallouts,
		liveMath,
		liveMermaid,
		liveFootnotes,
		liveEmbeds
	];
}

export class NoteEmbedCardWidget extends WidgetType {
	readonly target: EmbedTarget;
	readonly depth: number;
	readonly chain: readonly string[];
	readonly build: CardExtensions;
	private nested: EditorView | null = null;
	private alive = true;

	constructor(target: EmbedTarget, depth: number, chain: readonly string[], build: CardExtensions) {
		super();
		this.target = target;
		this.depth = depth;
		this.chain = chain;
		this.build = build;
	}

	eq(other: NoteEmbedCardWidget): boolean {
		return (
			other.target.abs === this.target.abs &&
			other.target.name === this.target.name &&
			other.target.heading === this.target.heading &&
			other.depth === this.depth &&
			other.chain.join('\u0000') === this.chain.join('\u0000')
		);
	}

	ignoreEvent(): boolean {
		return false;
	}

	toDOM(view: EditorView): HTMLElement {
		const ctx = contextOf(view.state);
		const card = document.createElement('div');
		card.className = 'cm-lp-note-embed';
		card.contentEditable = 'false';
		const header = document.createElement(this.target.abs ? 'button' : 'div');
		header.className = 'cm-lp-embed-header';
		header.appendChild(embedIcon('note'));
		const name = document.createElement('span');
		name.className = 'cm-lp-embed-name';
		name.textContent = this.target.name;
		header.appendChild(name);
		if (this.target.abs && header instanceof HTMLButtonElement) {
			header.type = 'button';
			header.title = m.editor_open_file({ name: this.target.name });
			header.addEventListener('mousedown', (event) => {
				event.preventDefault();
				event.stopPropagation();
			});
			header.addEventListener('click', (event) => {
				event.preventDefault();
				event.stopPropagation();
				ctx.openWikiLink(this.target.name);
			});
		}
		card.appendChild(header);
		const body = document.createElement('div');
		body.className = 'cm-lp-embed-body';
		card.appendChild(body);
		card.addEventListener('mousedown', (event) => {
			const target = event.target as HTMLElement | null;
			if (target?.closest('.cm-editor') || target?.closest('.cm-lp-embed-header')) return;
			event.preventDefault();
			event.stopPropagation();
			view.dispatch({ selection: { anchor: this.target.from } });
			view.focus();
		});
		void this.load(body, ctx);
		return card;
	}

	private async load(body: HTMLElement, ctx: LiveContext): Promise<void> {
		const fail = (text: string) => body.appendChild(messageElement('cm-lp-embed-missing', text));
		const abs = this.target.abs;
		if (!abs) {
			fail(m.note_embed_missing({ title: this.target.name }));
			return;
		}
		if (this.chain.includes(abs)) {
			fail(m.note_embed_load_failed({ title: this.target.name }));
			return;
		}
		try {
			const bytes = await readFileBytes(abs);
			if (!this.alive) return;
			const raw = new TextDecoder().decode(bytes);
			const text = this.target.heading ? headingSection(raw, this.target.heading) : raw;
			if (!text.trim()) {
				body.appendChild(messageElement('cm-lp-embed-empty', m.note_embed_empty()));
				return;
			}
			if (this.depth >= MAX_EMBED_DEPTH) {
				body.appendChild(messageElement('cm-lp-embed-plain', text));
				return;
			}
			this.nested = new EditorView({
				parent: body,
				state: EditorState.create({
					doc: text,
					extensions: this.build({ ...ctx, notePath: () => abs }, this.depth + 1, [
						...this.chain,
						abs
					])
				})
			});
		} catch {
			if (this.alive) fail(m.note_embed_load_failed({ title: this.target.name }));
		}
	}

	destroy(): void {
		this.alive = false;
		this.nested?.destroy();
		this.nested = null;
	}
}

export class NoteLinkWidget extends WidgetType {
	readonly target: EmbedTarget;

	constructor(target: EmbedTarget) {
		super();
		this.target = target;
	}

	eq(other: NoteLinkWidget): boolean {
		return other.target.abs === this.target.abs && other.target.name === this.target.name;
	}

	ignoreEvent(): boolean {
		return false;
	}

	toDOM(view: EditorView): HTMLElement {
		const ctx = contextOf(view.state);
		const chip = document.createElement('button');
		chip.type = 'button';
		chip.className = 'cm-lp-embed-chip';
		chip.title = m.editor_open_file({ name: this.target.name });
		chip.appendChild(embedIcon('note'));
		const name = document.createElement('span');
		name.textContent = this.target.name;
		chip.appendChild(name);
		if (this.target.abs) {
			chip.addEventListener('mousedown', (event) => {
				event.preventDefault();
				event.stopPropagation();
			});
			chip.addEventListener('click', (event) => {
				event.preventDefault();
				event.stopPropagation();
				ctx.openWikiLink(this.target.name);
			});
		} else {
			chip.classList.add('cm-lp-embed-missing');
		}
		return chip;
	}
}

export class FileCardWidget extends WidgetType {
	readonly target: EmbedTarget;

	constructor(target: EmbedTarget) {
		super();
		this.target = target;
	}

	eq(other: FileCardWidget): boolean {
		return other.target.abs === this.target.abs && other.target.name === this.target.name;
	}

	ignoreEvent(): boolean {
		return false;
	}

	toDOM(): HTMLElement {
		const card = document.createElement('button');
		card.type = 'button';
		card.className = 'cm-lp-file-card';
		card.appendChild(embedIcon('file'));
		const name = document.createElement('span');
		name.className = 'cm-lp-embed-name';
		name.textContent = this.target.name;
		card.appendChild(name);
		const size = document.createElement('span');
		size.className = 'cm-lp-file-size';
		card.appendChild(size);
		const abs = this.target.abs;
		if (!abs) {
			card.classList.add('cm-lp-embed-missing');
			size.textContent = m.file_embed_missing({ name: this.target.name });
			return card;
		}
		card.title = m.editor_open_file({ name: this.target.name });
		void fileMetadata(abs)
			.then((stats) => {
				if (stats.size != null) size.textContent = formatSize(stats.size);
			})
			.catch(() => undefined);
		card.addEventListener('mousedown', (event) => {
			event.preventDefault();
			event.stopPropagation();
		});
		card.addEventListener('click', (event) => {
			event.preventDefault();
			event.stopPropagation();
			void panes.openFile(abs);
		});
		return card;
	}
}
