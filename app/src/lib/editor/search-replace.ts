import { Extension, type Editor } from "@tiptap/core";
import { Plugin, PluginKey } from "@tiptap/pm/state";
import type { Node as PMNode } from "@tiptap/pm/model";
import { Decoration, DecorationSet } from "@tiptap/pm/view";
import type { MarkdownStorage } from "tiptap-markdown";
import { searchInText, type TextMatch } from "$lib/fs/bridge";

export interface SearchReplaceStorage {
  searchTerm: string;
  replaceTerm: string;
  caseSensitive: boolean;
  currentIndex: number;
  totalMatches: number;
}

declare module "@tiptap/core" {
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
    // tiptap-markdown's storage (it does not augment this interface itself).
    markdown: MarkdownStorage;
  }
}

interface SearchMatch {
  from: number;
  to: number;
}

const searchPluginKey = new PluginKey("searchReplace");

// ── Document flattening ──

/** Cached flattened document text + ProseMirror position mapping. */
let cachedDoc: PMNode | null = null;
let cachedFullText = "";
let cachedFullTextLower = "";
let cachedPmPos: number[] = [];
let cachedGaps: number[] = [];

function flattenDoc(doc: PMNode) {
  if (doc === cachedDoc) return;
  cachedDoc = doc;
  const chars: string[] = [];
  const pmPos: number[] = [];
  const gaps: number[] = [];
  doc.descendants((node: PMNode, pos: number) => {
    if (!node.isText) return;
    const t = node.text!;
    for (let i = 0; i < t.length; i++) {
      const charPos = pos + i;
      // Detect block boundary: PM position not consecutive with previous
      if (pmPos.length > 0 && charPos !== pmPos[pmPos.length - 1] + 1) {
        gaps.push(chars.length);
      }
      chars.push(t[i]);
      pmPos.push(charPos);
    }
  });
  cachedFullText = chars.join("");
  cachedFullTextLower = cachedFullText.toLowerCase();
  cachedPmPos = pmPos;
  cachedGaps = gaps;
}

// ── Async search (Rust IPC) ──

let searchVersion = 0;

/** Pending timer for doc-change-triggered re-search debounce. */
let docChangeSearchTimer: ReturnType<typeof setTimeout> | null = null;
const DOC_CHANGE_SEARCH_DELAY = 150;

/** Kick off an async Rust search and dispatch results back into the plugin. */
function triggerAsyncSearch(
  editor: Editor,
  doc: PMNode,
  searchTerm: string,
  caseSensitive: boolean,
  action: string,
) {
  if (!searchTerm) return;
  flattenDoc(doc);
  const version = ++searchVersion;

  searchInText(cachedFullText, cachedPmPos, cachedGaps, searchTerm, caseSensitive)
    .then((rustMatches: TextMatch[]) => {
      if (version !== searchVersion) return;
      const { tr } = editor.state;
      tr.setMeta(searchPluginKey, {
        action,
        asyncResults: rustMatches as SearchMatch[],
      });
      editor.view.dispatch(tr);
    })
    .catch(() => {
      // On IPC failure, fall back to sync JS search
      if (version !== searchVersion) return;
      const matches = findMatchesSync(doc, searchTerm, caseSensitive);
      const { tr } = editor.state;
      tr.setMeta(searchPluginKey, { action, asyncResults: matches });
      editor.view.dispatch(tr);
    });
}

// ── Sync JS fallback ──

function findMatchesSync(
  doc: PMNode,
  searchTerm: string,
  caseSensitive: boolean,
): SearchMatch[] {
  if (!searchTerm) return [];

  flattenDoc(doc);
  const matches: SearchMatch[] = [];
  const term = caseSensitive ? searchTerm : searchTerm.toLowerCase();
  const fullText = caseSensitive ? cachedFullText : cachedFullTextLower;
  const pmPos = cachedPmPos;

  let start = 0;
  while (start <= fullText.length - term.length) {
    const idx = fullText.indexOf(term, start);
    if (idx === -1) break;

    let crossBlock = false;
    for (let k = idx + 1; k < idx + term.length; k++) {
      if (pmPos[k] !== pmPos[k - 1] + 1) {
        crossBlock = true;
        break;
      }
    }

    if (!crossBlock) {
      matches.push({ from: pmPos[idx], to: pmPos[idx + term.length - 1] + 1 });
    }

    start = idx + 1;
  }

  return matches;
}

// ── Plugin state helpers ──

interface PluginState {
  baseDecos: DecorationSet;
  currentDeco: DecorationSet;
  matches: SearchMatch[];
  /** Doc reference the matches were computed for — used to verify freshness. */
  matchDoc: PMNode | null;
}

function buildBaseDecos(doc: PMNode, matches: SearchMatch[]): DecorationSet {
  if (matches.length === 0) return DecorationSet.empty;
  const decorations = matches.map((match) =>
    Decoration.inline(match.from, match.to, { class: "search-match" }),
  );
  return DecorationSet.create(doc, decorations);
}

function buildCurrentDeco(
  doc: PMNode,
  matches: SearchMatch[],
  currentIndex: number,
): DecorationSet {
  if (matches.length === 0 || currentIndex >= matches.length)
    return DecorationSet.empty;
  const match = matches[currentIndex];
  return DecorationSet.create(doc, [
    Decoration.inline(match.from, match.to, { class: "search-match-current" }),
  ]);
}

// ── Extension ──

export const SearchReplace = Extension.create<{}, SearchReplaceStorage>({
  name: "searchReplace",

  addStorage() {
    return {
      searchTerm: "",
      replaceTerm: "",
      caseSensitive: false,
      currentIndex: 0,
      totalMatches: 0,
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
          tr.setMeta(searchPluginKey, { action: "update" });
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
          tr.setMeta(searchPluginKey, { action: "update" });
          editor.view.dispatch(tr);
          return true;
        },
      findNext:
        () =>
        ({ editor }) => {
          const storage = editor.storage.searchReplace;
          if (storage.totalMatches === 0) return false;
          storage.currentIndex =
            (storage.currentIndex + 1) % storage.totalMatches;
          const { tr } = editor.state;
          tr.setMeta(searchPluginKey, { action: "navigate" });
          editor.view.dispatch(tr);
          return true;
        },
      findPrev:
        () =>
        ({ editor }) => {
          const storage = editor.storage.searchReplace;
          if (storage.totalMatches === 0) return false;
          storage.currentIndex =
            (storage.currentIndex - 1 + storage.totalMatches) %
            storage.totalMatches;
          const { tr } = editor.state;
          tr.setMeta(searchPluginKey, { action: "navigate" });
          editor.view.dispatch(tr);
          return true;
        },
      replaceCurrent:
        () =>
        ({ editor }) => {
          const storage = editor.storage.searchReplace;
          if (storage.totalMatches === 0) return false;

          // Use cached matches if fresh, else sync JS fallback
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
              // Collect marks from the matched range so we preserve formatting
              const resolvedFrom = tr.doc.resolve(match.from);
              const marks = resolvedFrom.marksAcross(tr.doc.resolve(match.to)) ?? resolvedFrom.marks();
              const schema = editor.state.schema;
              const replaceNode = schema.text(storage.replaceTerm, marks);
              tr.replaceWith(match.from, match.to, replaceNode);
              tr.setMeta(searchPluginKey, { action: "update" });
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
              tr.setMeta(searchPluginKey, { action: "update" });
              return true;
            })
            .run();

          return true;
        },
      clearSearch:
        () =>
        ({ editor }) => {
          const storage = editor.storage.searchReplace;
          storage.searchTerm = "";
          storage.replaceTerm = "";
          storage.currentIndex = 0;
          storage.totalMatches = 0;
          ++searchVersion; // cancel any pending async search
          if (docChangeSearchTimer) {
            clearTimeout(docChangeSearchTimer);
            docChangeSearchTimer = null;
          }
          const { tr } = editor.state;
          tr.setMeta(searchPluginKey, { action: "update" });
          editor.view.dispatch(tr);
          return true;
        },
    };
  },

  addProseMirrorPlugins() {
    const extensionThis = this;

    return [
      new Plugin<PluginState>({
        key: searchPluginKey,
        state: {
          init(): PluginState {
            return { baseDecos: DecorationSet.empty, currentDeco: DecorationSet.empty, matches: [], matchDoc: null };
          },
          apply(tr, prev, _oldState, newState): PluginState {
            const meta = tr.getMeta(searchPluginKey);
            const docChanged = tr.docChanged;

            // Async results arrived
            if (meta?.asyncResults) {
              const matches = meta.asyncResults as SearchMatch[];
              const storage = extensionThis.storage;
              storage.totalMatches = matches.length;
              if (storage.currentIndex >= matches.length) {
                storage.currentIndex = 0;
              }
              const baseDecos = buildBaseDecos(newState.doc, matches);
              const currentDeco = buildCurrentDeco(newState.doc, matches, storage.currentIndex);

              if (meta.action === "navigate" || meta.action === "update") {
                scrollToMatch(matches, storage.currentIndex);
              }

              return { baseDecos, currentDeco, matches, matchDoc: newState.doc };
            }

            if (!meta && !docChanged) return prev;

            const storage = extensionThis.storage;

            // Navigate: rebuild current-match deco
            if (meta?.action === "navigate" && prev.matchDoc === newState.doc) {
              const currentDeco = buildCurrentDeco(newState.doc, prev.matches, storage.currentIndex);
              scrollToMatch(prev.matches, storage.currentIndex);
              return { ...prev, currentDeco };
            }

            // Term/doc changed: async re-search
            if (storage.searchTerm) {
              if (meta) {
                // Explicit command (update/navigate): search immediately.
                if (docChangeSearchTimer) {
                  clearTimeout(docChangeSearchTimer);
                  docChangeSearchTimer = null;
                }
                triggerAsyncSearch(
                  extensionThis.editor,
                  newState.doc,
                  storage.searchTerm,
                  storage.caseSensitive,
                  meta.action ?? "update",
                );
              } else {
                // Pure doc edit: debounce so rapid typing collapses to one IPC.
                ++searchVersion; // invalidate any in-flight search result
                if (docChangeSearchTimer) clearTimeout(docChangeSearchTimer);
                docChangeSearchTimer = setTimeout(() => {
                  docChangeSearchTimer = null;
                  const editor = extensionThis.editor;
                  const liveStorage = extensionThis.storage;
                  if (!editor || !liveStorage.searchTerm) return;
                  triggerAsyncSearch(
                    editor,
                    editor.state.doc,
                    liveStorage.searchTerm,
                    liveStorage.caseSensitive,
                    "update",
                  );
                }, DOC_CHANGE_SEARCH_DELAY);
              }
            } else {
              storage.totalMatches = 0;
              return { baseDecos: DecorationSet.empty, currentDeco: DecorationSet.empty, matches: [], matchDoc: newState.doc };
            }

            // Map stale decos while async search runs
            if (docChanged && prev.baseDecos !== DecorationSet.empty) {
              return {
                ...prev,
                baseDecos: prev.baseDecos.map(tr.mapping, newState.doc),
                currentDeco: prev.currentDeco.map(tr.mapping, newState.doc),
              };
            }
            return prev;
          },
        },
        props: {
          decorations(state) {
            const ps = this.getState(state) as PluginState | undefined;
            if (!ps || (ps.baseDecos === DecorationSet.empty && ps.currentDeco === DecorationSet.empty)) {
              return DecorationSet.empty;
            }
            if (ps.currentDeco === DecorationSet.empty) return ps.baseDecos;
            return ps.baseDecos.add(state.doc, ps.currentDeco.find());
          },
        },
      }),
    ];

    let scrollRaf = 0;

    function scrollToMatch(matches: SearchMatch[], currentIndex: number) {
      const current = matches[currentIndex];
      if (!current) return;
      // Coalesce rapid next/prev into a single scroll on the next frame.
      if (scrollRaf) cancelAnimationFrame(scrollRaf);
      scrollRaf = requestAnimationFrame(() => {
        scrollRaf = 0;
        try {
          const view = extensionThis.editor.view;
          const coords = view.coordsAtPos(current.from);
          const scrollContainer = view.dom.closest(".editor-container");
          const editorRect = scrollContainer?.getBoundingClientRect();
          if (editorRect && coords && scrollContainer) {
            const relativeTop =
              coords.top - editorRect.top + scrollContainer.scrollTop;
            const middle = relativeTop - editorRect.height / 2;
            scrollContainer.scrollTo({ top: middle, behavior: "smooth" });
          }
        } catch {
          /* ignore scroll errors */
        }
      });
    }
  },
});
