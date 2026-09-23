import { Extension, type Editor } from '@tiptap/core';
import { Plugin, PluginKey } from '@tiptap/pm/state';
import type { Node as PMNode } from '@tiptap/pm/model';
import { DecorationSet } from '@tiptap/pm/view';
import type { MarkdownStorage } from 'tiptap-markdown';
import { searchInText, type TextMatch } from '$lib/fs/bridge';
import { findMatchesSync, flattenDoc, type SearchMatch } from '$lib/editor/search-text';
import { buildBaseDecos, buildCurrentDeco } from '$lib/editor/search-decorations';

export interface SearchReplaceStorage {
	searchTerm: string;
	replaceTerm: string;
	caseSensitive: boolean;
	currentIndex: number;
	totalMatches: number;
}

declare module '@tiptap/core' {
	interface Commands<ReturnType> {
		searchReplace: {
			setSearchTerm: (term: string) => ReturnType;
			setReplaceTerm: (term: string) => ReturnType;
			setCaseSensitive: (value: boolean) => ReturnType;
			findNext: () => ReturnType;
			findPrev: () => ReturnType;
			replaceCurrent: () => ReturnType;
			replaceAll: () => ReturnType;
			clearSearch: () => ReturnType;
		};
	}

	interface Storage {
		searchReplace: SearchReplaceStorage;
		markdown: MarkdownStorage;
	}
}

const searchPluginKey = new PluginKey('searchReplace');

let searchVersion = 0;
let docChangeSearchTimer: ReturnType<typeof setTimeout> | null = null;
const DOC_CHANGE_SEARCH_DELAY = 150;

function triggerAsyncSearch(
	editor: Editor,
	doc: PMNode,
	searchTerm: string,
	caseSensitive: boolean,
	action: string
) {
	if (!searchTerm) return;
	const { text, pos, gaps } = flattenDoc(doc);
	const version = ++searchVersion;

	searchInText(text, pos, gaps, searchTerm, caseSensitive)
		.then((rustMatches: TextMatch[]) => {
			if (version !== searchVersion) return;
			const { tr } = editor.state;
			tr.setMeta(searchPluginKey, {
				action,
				asyncResults: rustMatches as SearchMatch[]
			});
			editor.view.dispatch(tr);
		})
		.catch(() => {
			if (version !== searchVersion) return;
			const matches = findMatchesSync(doc, searchTerm, caseSensitive);
			const { tr } = editor.state;
			tr.setMeta(searchPluginKey, { action, asyncResults: matches });
			editor.view.dispatch(tr);
		});
}

interface PluginState {
	baseDecos: DecorationSet;
	currentDeco: DecorationSet;
	matches: SearchMatch[];
	matchDoc: PMNode | null;
}

export const SearchReplace = Extension.create<Record<string, never>, SearchReplaceStorage>({
	name: 'searchReplace',

	addStorage() {
		return {
			searchTerm: '',
			replaceTerm: '',
			caseSensitive: false,
			currentIndex: 0,
			totalMatches: 0
		};
	},

	addCommands() {
		return {
			setSearchTerm:
				(term: string) =>
				({ editor }) => {
					editor.storage.searchReplace.searchTerm = term;
					editor.storage.searchReplace.currentIndex = 0;
					const { tr } = editor.state;
					tr.setMeta(searchPluginKey, { action: 'update' });
					editor.view.dispatch(tr);
					return true;
				},
			setReplaceTerm:
				(term: string) =>
				({ editor }) => {
					editor.storage.searchReplace.replaceTerm = term;
					return true;
				},
			setCaseSensitive:
				(value: boolean) =>
				({ editor }) => {
					editor.storage.searchReplace.caseSensitive = value;
					editor.storage.searchReplace.currentIndex = 0;
					const { tr } = editor.state;
					tr.setMeta(searchPluginKey, { action: 'update' });
					editor.view.dispatch(tr);
					return true;
				},
			findNext:
				() =>
				({ editor }) => {
					const storage = editor.storage.searchReplace;
					if (storage.totalMatches === 0) return false;
					storage.currentIndex = (storage.currentIndex + 1) % storage.totalMatches;
					const { tr } = editor.state;
					tr.setMeta(searchPluginKey, { action: 'navigate' });
					editor.view.dispatch(tr);
					return true;
				},
			findPrev:
				() =>
				({ editor }) => {
					const storage = editor.storage.searchReplace;
					if (storage.totalMatches === 0) return false;
					storage.currentIndex =
						(storage.currentIndex - 1 + storage.totalMatches) % storage.totalMatches;
					const { tr } = editor.state;
					tr.setMeta(searchPluginKey, { action: 'navigate' });
					editor.view.dispatch(tr);
					return true;
				},
			replaceCurrent:
				() =>
				({ editor }) => {
					const storage = editor.storage.searchReplace;
					if (storage.totalMatches === 0) return false;

					const pluginState = searchPluginKey.getState(editor.state) as PluginState | undefined;
					const matches =
						pluginState && pluginState.matchDoc === editor.state.doc
							? pluginState.matches
							: findMatchesSync(editor.state.doc, storage.searchTerm, storage.caseSensitive);
					if (matches.length === 0) return false;

					const match = matches[storage.currentIndex];
					if (!match) return false;

					editor
						.chain()
						.command(({ tr }) => {
							const resolvedFrom = tr.doc.resolve(match.from);
							const marks =
								resolvedFrom.marksAcross(tr.doc.resolve(match.to)) ?? resolvedFrom.marks();
							const schema = editor.state.schema;
							const replaceNode = schema.text(storage.replaceTerm, marks);
							tr.replaceWith(match.from, match.to, replaceNode);
							tr.setMeta(searchPluginKey, { action: 'update' });
							return true;
						})
						.run();

					return true;
				},
			replaceAll:
				() =>
				({ editor }) => {
					const storage = editor.storage.searchReplace;
					if (storage.totalMatches === 0) return false;

					const pluginState = searchPluginKey.getState(editor.state) as PluginState | undefined;
					const matches =
						pluginState && pluginState.matchDoc === editor.state.doc
							? pluginState.matches
							: findMatchesSync(editor.state.doc, storage.searchTerm, storage.caseSensitive);
					if (matches.length === 0) return false;

					editor
						.chain()
						.command(({ tr }) => {
							const schema = editor.state.schema;
							for (let i = matches.length - 1; i >= 0; i--) {
								const from = matches[i].from;
								const to = matches[i].to;
								const resolvedFrom = tr.doc.resolve(from);
								const marks = resolvedFrom.marksAcross(tr.doc.resolve(to)) ?? resolvedFrom.marks();
								const replaceNode = schema.text(storage.replaceTerm, marks);
								tr.replaceWith(from, to, replaceNode);
							}
							tr.setMeta(searchPluginKey, { action: 'update' });
							return true;
						})
						.run();

					return true;
				},
			clearSearch:
				() =>
				({ editor }) => {
					const storage = editor.storage.searchReplace;
					storage.searchTerm = '';
					storage.replaceTerm = '';
					storage.currentIndex = 0;
					storage.totalMatches = 0;
					++searchVersion;
					if (docChangeSearchTimer) {
						clearTimeout(docChangeSearchTimer);
						docChangeSearchTimer = null;
					}
					const { tr } = editor.state;
					tr.setMeta(searchPluginKey, { action: 'update' });
					editor.view.dispatch(tr);
					return true;
				}
		};
	},

	addProseMirrorPlugins() {
		// Must stay above the `return`: the plugin's `apply` reads `scrollRaf`.
		let scrollRaf = 0;

		const SCROLL_EDGE_MARGIN = 64;

		const scrollToMatch = (matches: SearchMatch[], currentIndex: number) => {
			const current = matches[currentIndex];
			if (!current) return;
			if (scrollRaf) cancelAnimationFrame(scrollRaf);
			scrollRaf = requestAnimationFrame(() => {
				scrollRaf = 0;
				const editor = this.editor;
				if (!editor || editor.isDestroyed) return;
				try {
					const view = editor.view;
					if (current.to > view.state.doc.content.size) return;
					const scrollContainer = view.dom.closest('.editor-container');
					if (!scrollContainer) return;
					const coords = view.coordsAtPos(current.from);
					const box = scrollContainer.getBoundingClientRect();
					if (
						coords.top >= box.top + SCROLL_EDGE_MARGIN &&
						coords.bottom <= box.bottom - SCROLL_EDGE_MARGIN
					) {
						return;
					}
					const relativeTop = coords.top - box.top + scrollContainer.scrollTop;
					scrollContainer.scrollTo({
						top: Math.max(0, relativeTop - box.height / 2),
						behavior: 'smooth'
					});
				} catch {
					return;
				}
			});
		};

		return [
			new Plugin<PluginState>({
				key: searchPluginKey,
				state: {
					init(): PluginState {
						return {
							baseDecos: DecorationSet.empty,
							currentDeco: DecorationSet.empty,
							matches: [],
							matchDoc: null
						};
					},
					apply: (tr, prev, _oldState, newState): PluginState => {
						const meta = tr.getMeta(searchPluginKey);
						const docChanged = tr.docChanged;

						if (meta?.asyncResults) {
							const matches = meta.asyncResults as SearchMatch[];
							const storage = this.storage;
							storage.totalMatches = matches.length;
							if (storage.currentIndex >= matches.length) {
								storage.currentIndex = 0;
							}
							const baseDecos = buildBaseDecos(newState.doc, matches);
							const currentDeco = buildCurrentDeco(newState.doc, matches, storage.currentIndex);

							if (meta.action === 'navigate' || meta.action === 'update') {
								scrollToMatch(matches, storage.currentIndex);
							}

							return { baseDecos, currentDeco, matches, matchDoc: newState.doc };
						}

						if (!meta && !docChanged) return prev;

						const storage = this.storage;

						if (meta?.action === 'navigate' && prev.matchDoc === newState.doc) {
							const currentDeco = buildCurrentDeco(
								newState.doc,
								prev.matches,
								storage.currentIndex
							);
							scrollToMatch(prev.matches, storage.currentIndex);
							return { ...prev, currentDeco };
						}

						if (storage.searchTerm) {
							if (meta) {
								if (docChangeSearchTimer) {
									clearTimeout(docChangeSearchTimer);
									docChangeSearchTimer = null;
								}
								triggerAsyncSearch(
									this.editor,
									newState.doc,
									storage.searchTerm,
									storage.caseSensitive,
									meta.action ?? 'update'
								);
							} else {
								++searchVersion;
								if (docChangeSearchTimer) clearTimeout(docChangeSearchTimer);
								docChangeSearchTimer = setTimeout(() => {
									docChangeSearchTimer = null;
									const editor = this.editor;
									const liveStorage = this.storage;
									if (!editor || !liveStorage.searchTerm) return;
									triggerAsyncSearch(
										editor,
										editor.state.doc,
										liveStorage.searchTerm,
										liveStorage.caseSensitive,
										'refresh'
									);
								}, DOC_CHANGE_SEARCH_DELAY);
							}
						} else {
							storage.totalMatches = 0;
							return {
								baseDecos: DecorationSet.empty,
								currentDeco: DecorationSet.empty,
								matches: [],
								matchDoc: newState.doc
							};
						}

						if (docChanged && prev.baseDecos !== DecorationSet.empty) {
							return {
								...prev,
								baseDecos: prev.baseDecos.map(tr.mapping, newState.doc),
								currentDeco: prev.currentDeco.map(tr.mapping, newState.doc)
							};
						}
						return prev;
					}
				},
				props: {
					decorations(state) {
						const ps = this.getState(state) as PluginState | undefined;
						if (
							!ps ||
							(ps.baseDecos === DecorationSet.empty && ps.currentDeco === DecorationSet.empty)
						) {
							return DecorationSet.empty;
						}
						if (ps.currentDeco === DecorationSet.empty) return ps.baseDecos;
						return ps.baseDecos.add(state.doc, ps.currentDeco.find());
					}
				}
			})
		];
	}
});
