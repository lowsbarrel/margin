import {
	acceptCompletion,
	autocompletion,
	closeCompletion,
	completionStatus,
	pickedCompletion,
	type Completion,
	type CompletionContext,
	type CompletionResult,
	type CompletionSource
} from '@codemirror/autocomplete';
import { syntaxTree } from '@codemirror/language';
import { Prec, type EditorState } from '@codemirror/state';
import type { SyntaxNode } from '@lezer/common';
import { ViewPlugin, keymap, type EditorView } from '@codemirror/view';
import { commands, type FuzzyEntry } from '$lib/bindings';
import { tags as tagsStore } from '$lib/stores/tags.svelte';
import { vault } from '$lib/stores/vault.svelte';
import { isImageFile } from '$lib/utils/mime';
import { assistCompletion, assistCompletionConfig, assistTheme } from './complete-theme';
import { contextOf } from './context';
import { ESCAPE_COMPLETION, onEscape } from './escape';
import { headingsOfNote } from './note-headings';
import { fuzzyMatch, filterSlashItems, type SlashItem } from './slash-items';
import { FRONTMATTER } from './syntax';
import { listVaultFiles } from './vault-index';

const SLASH = /(?:^|\s)\/(\S*)$/;
const WIKI = /(^|[^[\]])(!?)\[\[([^[\]\n|#]*)(?:#([^[\]\n|]*))?$/;
const TAG = /(^|[^\w#/])#([a-zA-Z][\w/-]*)$/;
const NOTE = /\.(md|canvas)$/i;
const HEADING_LINE = /^#{1,6}(?:[ \t]|$)/;
const LITERAL_NODES: Record<string, true> = {
	FencedCode: true,
	CodeBlock: true,
	InlineCode: true,
	[FRONTMATTER]: true
};

function inLiteral(state: EditorState, pos: number): boolean {
	let node: SyntaxNode | null = syntaxTree(state).resolveInner(Math.max(0, pos - 1), -1);
	while (node) {
		if (LITERAL_NODES[node.name]) return true;
		node = node.parent;
	}
	return false;
}

function rank(names: string[], needle: string): string[] {
	if (!needle) return names;
	return names
		.map((name) => ({ name, match: fuzzyMatch(needle.toLowerCase(), name) }))
		.filter((entry) => entry.match !== null)
		.sort((a, b) => (b.match?.score ?? 0) - (a.match?.score ?? 0))
		.map((entry) => entry.name);
}

function matches(needle: string) {
	return (completion: Completion): readonly number[] =>
		fuzzyMatch(needle.toLowerCase(), completion.label)?.ranges ?? [];
}

function insertWiki(
	view: EditorView,
	completion: Completion,
	from: number,
	to: number,
	text: string
): void {
	const doc = view.state.doc;
	const closed = doc.sliceString(to, Math.min(to + 2, doc.length)) === ']]';
	const insert = closed ? text : `${text}]]`;
	view.dispatch({
		changes: { from, to, insert },
		selection: { anchor: from + insert.length + (closed ? 2 : 0) },
		annotations: pickedCompletion.of(completion),
		userEvent: 'input.complete',
		scrollIntoView: true
	});
}

function slashSource(context: CompletionContext): CompletionResult | null {
	const { state, pos } = context;
	const line = state.doc.lineAt(pos);
	const match = SLASH.exec(state.sliceDoc(line.from, pos));
	if (!match || inLiteral(state, pos)) return null;
	const query = match[1];
	const from = pos - query.length - 1;
	const items: SlashItem[] = filterSlashItems(query);
	return {
		from,
		filter: false,
		getMatch: matches(query),
		options: items.map((item) =>
			assistCompletion(item.title, item.description, item.icon, {
				apply: (view, completion, at, to) => {
					const edit = item.edit(view.state, at, to);
					view.dispatch({
						changes: edit.changes,
						selection: { anchor: edit.selection },
						annotations: pickedCompletion.of(completion),
						userEvent: 'input.complete',
						scrollIntoView: true
					});
				}
			})
		)
	};
}

function noteRelativePath(state: EditorState, target: string): string | null {
	if (!target) return null;
	const ctx = contextOf(state);
	const name = NOTE.test(target) ? target : `${target}.md`;
	return ctx.findByName(name) ?? ctx.findByName(target) ?? (ctx.exists(name) ? name : null);
}

function vaultEntries(): FuzzyEntry[] {
	return listVaultFiles().map((path) => ({
		name: path.slice(path.lastIndexOf('/') + 1),
		path
	}));
}

function wikiCompletion(entry: FuzzyEntry): Completion {
	const slash = entry.path.lastIndexOf('/');
	const folder = slash < 0 ? '' : entry.path.slice(0, slash);
	const name = entry.name.replace(/\.(md|canvas)$/i, '');
	return assistCompletion(name, folder, isImageFile(entry.name) ? '🖼' : '📄', {
		apply: (view, completion, from, to) => insertWiki(view, completion, from, to, name)
	});
}

async function wikiSource(context: CompletionContext): Promise<CompletionResult | null> {
	const { state, pos } = context;
	const line = state.doc.lineAt(pos);
	const match = WIKI.exec(state.sliceDoc(line.from, pos));
	if (!match || inLiteral(state, pos)) return null;
	const embed = match[2] === '!';
	const target = match[3];
	const heading = match[4];

	if (heading !== undefined) {
		const rel = noteRelativePath(state, target);
		const vaultPath = vault.vaultPath;
		if (!rel || !vaultPath) return null;
		return {
			from: pos - heading.length,
			filter: false,
			getMatch: matches(heading),
			options: rank(await headingsOfNote(`${vaultPath}/${rel}`), heading).map((name) =>
				assistCompletion(name, '', '§', {
					apply: (view, completion, at, to) => insertWiki(view, completion, at, to, name)
				})
			)
		};
	}

	const from = pos - target.length;
	const entries = vaultEntries();
	if (!entries.length) return null;
	const notes = entries.filter((entry) => NOTE.test(entry.path));
	const groups = embed ? [entries.filter((entry) => !NOTE.test(entry.path)), notes] : [notes];
	const results: FuzzyEntry[] = [];
	for (const group of groups) {
		if (!group.length) continue;
		results.push(...(await commands.fuzzyFilterFiles(group, target, 25)));
	}
	if (!results.length) return null;
	return {
		from,
		filter: false,
		getMatch: matches(target),
		options: results.map(wikiCompletion)
	};
}

async function tagSource(context: CompletionContext): Promise<CompletionResult | null> {
	const { state, pos } = context;
	const line = state.doc.lineAt(pos);
	if (HEADING_LINE.test(line.text) || inLiteral(state, pos)) return null;
	const match = TAG.exec(state.sliceDoc(line.from, pos));
	if (!match) return null;
	const vaultPath = vault.vaultPath;
	if (!vaultPath) return null;
	const query = match[2];
	const found = await tagsStore.load(vaultPath);
	return {
		from: pos - query.length,
		filter: false,
		getMatch: matches(query),
		options: rank(
			found.map((entry) => entry.tag),
			query
		)
			.slice(0, 30)
			.map((tag) =>
				assistCompletion(`#${tag}`, '', '', {
					apply: (view, completion, from, to) => {
						view.dispatch({
							changes: { from, to, insert: tag },
							selection: { anchor: from + tag.length },
							annotations: pickedCompletion.of(completion),
							userEvent: 'input.complete',
							scrollIntoView: true
						});
					}
				})
			)
	};
}

const sources: readonly CompletionSource[] = [slashSource, wikiSource, tagSource];

// Escape must close the menu before any other feature claims the key.
const escapeClosesMenu = ViewPlugin.fromClass(
	class {
		release: () => void;

		constructor(readonly view: EditorView) {
			this.release = onEscape(view, ESCAPE_COMPLETION, () => {
				if (completionStatus(view.state) === null) return false;
				closeCompletion(view);
				return true;
			});
		}

		destroy() {
			this.release();
		}
	}
);

export const liveAssist = [
	autocompletion({ ...assistCompletionConfig, override: sources }),
	escapeClosesMenu,
	assistTheme,
	// `acceptCompletion` yields to the list-indent binding when no menu is open.
	Prec.high(keymap.of([{ key: 'Tab', run: acceptCompletion }]))
];
