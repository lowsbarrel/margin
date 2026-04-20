import { Extension } from "@tiptap/core";
import { Plugin, PluginKey } from "@tiptap/pm/state";
import { Decoration, DecorationSet } from "@tiptap/pm/view";
import { searchInText, type TextMatch } from "$lib/fs/bridge";

export interface SearchReplaceStorage {
  searchTerm: string;
  replaceTerm: string;
  caseSensitive: boolean;
  currentIndex: number;
  totalMatches: number;
}

interface SearchMatch {
  from: number;
  to: number;
}

const searchPluginKey = new PluginKey("searchReplace");

// ─── Document flattening (must stay in JS — needs ProseMirror API) ───────────

/** Cached flattened document text + ProseMirror position mapping. */
let cachedDoc: any = null;
let cachedFullText = "";
let cachedPmPos: number[] = [];
let cachedGaps: number[] = [];

function flattenDoc(doc: any) {
  if (doc === cachedDoc) return;
  cachedDoc = doc;
  const chars: string[] = [];
  const pmPos: number[] = [];
  const gaps: number[] = [];
  doc.descendants((node: any, pos: number) => {
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
  cachedPmPos = pmPos;
  cachedGaps = gaps;
}

// ─── Rust-accelerated async search ───────────────────────────────────────────

let searchVersion = 0;

/** Kick off an async Rust search and dispatch results back into the plugin. */
function triggerAsyncSearch(
  editor: any,
  doc: any,
  searchTerm: string,
  caseSensitive: boolean,
  action: string,
) {
  if (!searchTerm) return;
  flattenDoc(doc);
  const version = ++searchVersion;

  searchInText(cachedFullText, cachedPmPos, cachedGaps, searchTerm, caseSensitive)
    .then((rustMatches: TextMatch[]) => {
      // Discard if a newer search was started
      if (version !== searchVersion) return;
      // Dispatch results back into the PM plugin
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

// ─── Sync JS fallback (for replace commands that need immediate results) ─────

function findMatchesSync(
  doc: any,
  searchTerm: string,
  caseSensitive: boolean,
): SearchMatch[] {
  if (!searchTerm) return [];

  flattenDoc(doc);
  const matches: SearchMatch[] = [];
  const term = caseSensitive ? searchTerm : searchTerm.toLowerCase();
  const fullText = caseSensitive ? cachedFullText : cachedFullText.toLowerCase();
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

// ─── Plugin state helpers ────────────────────────────────────────────────────

interface PluginState {
  baseDecos: DecorationSet;
  currentDeco: DecorationSet;
  matches: SearchMatch[];
  /** Doc reference the matches were computed for — used to verify freshness. */
  matchDoc: any;
}

function buildBaseDecos(doc: any, matches: SearchMatch[]): DecorationSet {
  if (matches.length === 0) return DecorationSet.empty;
  const decorations = matches.map((match) =>
    Decoration.inline(match.from, match.to, { class: "search-match" }),
  );
  return DecorationSet.create(doc, decorations);
}

function buildCurrentDeco(
  doc: any,
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

// ─── Extension ───────────────────────────────────────────────────────────────

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

  addCommands(): any {
    return {
      setSearchTerm:
        (term: string) =>
        ({ editor }: any) => {
          editor.storage.searchReplace.searchTerm = term;
          editor.storage.searchReplace.currentIndex = 0;
          const { tr } = editor.state;
          tr.setMeta(searchPluginKey, { action: "update" });
          editor.view.dispatch(tr);
          return true;
        },
      setReplaceTerm:
        (term: string) =>
        ({ editor }: any) => {
          editor.storage.searchReplace.replaceTerm = term;
          return true;
        },
      setCaseSensitive:
        (value: boolean) =>
        ({ editor }: any) => {
          editor.storage.searchReplace.caseSensitive = value;
          editor.storage.searchReplace.currentIndex = 0;
          const { tr } = editor.state;
          tr.setMeta(searchPluginKey, { action: "update" });
          editor.view.dispatch(tr);
          return true;
        },
      findNext:
        () =>
        ({ editor }: any) => {
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
        ({ editor }: any) => {
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
        ({ editor }: any) => {
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
            .command(({ tr }: any) => {
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
        ({ editor }: any) => {
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
            .command(({ tr }: any) => {
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
        ({ editor }: any) => {
          const storage = editor.storage.searchReplace;
          storage.searchTerm = "";
          storage.replaceTerm = "";
          storage.currentIndex = 0;
          storage.totalMatches = 0;
          ++searchVersion; // cancel any pending async search
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

            // ── Async results arrived from Rust ──
            if (meta?.asyncResults) {
              const matches = meta.asyncResults as SearchMatch[];
              const storage = extensionThis.storage;
              storage.totalMatches = matches.length;
              if (storage.currentIndex >= matches.length) {
                storage.currentIndex = 0;
              }
              const baseDecos = buildBaseDecos(newState.doc, matches);
              const currentDeco = buildCurrentDeco(newState.doc, matches, storage.currentIndex);

              // Scroll to current match
              if (meta.action === "navigate" || meta.action === "update") {
                scrollToMatch(matches, storage.currentIndex);
              }

              return { baseDecos, currentDeco, matches, matchDoc: newState.doc };
            }

            if (!meta && !docChanged) return prev;

            const storage = extensionThis.storage;

            // ── Navigation: only rebuild the single current-match decoration ──
            if (meta?.action === "navigate" && prev.matchDoc === newState.doc) {
              const currentDeco = buildCurrentDeco(newState.doc, prev.matches, storage.currentIndex);
              scrollToMatch(prev.matches, storage.currentIndex);
              return { ...prev, currentDeco };
            }

            // ── Search term changed or doc changed: kick off async Rust search ──
            if (storage.searchTerm) {
              triggerAsyncSearch(
                extensionThis.editor,
                newState.doc,
                storage.searchTerm,
                storage.caseSensitive,
                meta?.action ?? "update",
              );
            } else {
              storage.totalMatches = 0;
              return { baseDecos: DecorationSet.empty, currentDeco: DecorationSet.empty, matches: [], matchDoc: newState.doc };
            }

            // Return stale decorations while async search is in flight.
            // If doc changed, try to map old decorations to new positions.
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

    function scrollToMatch(matches: SearchMatch[], currentIndex: number) {
      const current = matches[currentIndex];
      if (!current) return;
      setTimeout(() => {
        try {
          const view = extensionThis.editor.view;
          const coords = view.coordsAtPos(current.from);
          const editorRect = view.dom
            .closest(".editor-container")
            ?.getBoundingClientRect();
          if (editorRect && coords) {
            const scrollContainer = view.dom.closest(".editor-container");
            if (scrollContainer) {
              const relativeTop =
                coords.top - editorRect.top + scrollContainer.scrollTop;
              const middle = relativeTop - editorRect.height / 2;
              scrollContainer.scrollTo({ top: middle, behavior: "smooth" });
            }
          }
        } catch {
          /* ignore scroll errors */
        }
      }, 10);
    }
  },
});
