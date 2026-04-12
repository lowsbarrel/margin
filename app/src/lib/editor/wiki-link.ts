import { Node, mergeAttributes, InputRule } from "@tiptap/core";
import { Plugin, PluginKey } from "@tiptap/pm/state";
import type { NodeType } from "@tiptap/pm/model";
import { extractWikiLinks, type TextNode } from "$lib/fs/bridge";

declare module "@tiptap/core" {
  interface Commands<ReturnType> {
    wikiLink: {
      insertWikiLink: (title: string) => ReturnType;
    };
  }
}

const WikiLink = Node.create({
  name: "wikiLink",
  group: "inline",
  inline: true,
  atom: true,
  selectable: true,

  addAttributes() {
    return {
      title: {
        default: null,
        parseHTML: (el) => (el as HTMLElement).getAttribute("data-title"),
        renderHTML: (attrs) => ({ "data-title": attrs.title }),
      },
    };
  },

  parseHTML() {
    return [{ tag: "span[data-wiki-link]" }];
  },

  renderHTML({ node, HTMLAttributes }) {
    return [
      "span",
      mergeAttributes(
        { "data-wiki-link": "", class: "wiki-link" },
        HTMLAttributes,
      ),
      `[[${node.attrs.title}]]`,
    ];
  },

  addCommands() {
    return {
      insertWikiLink:
        (title: string) =>
        ({ chain }) => {
          return chain()
            .focus()
            .insertContent({ type: this.name, attrs: { title } })
            .run();
        },
    };
  },

  addInputRules() {
    return [
      new InputRule({
        find: /(?<!!)\[\[([^\]\[\n]+)\]\]$/,
        handler: ({ range, match, chain }) => {
          const title = match[1];
          if (!title) return null;
          chain()
            .deleteRange(range)
            .insertContent({ type: this.name, attrs: { title } })
            .run();
        },
      }),
    ];
  },

  /** Convert plain-text [[title]] to WikiLink nodes on first mount */
  onCreate() {
    const nodeType = this.type;
    const editor = this.editor;
    // Defer so the editor view is fully ready to accept transactions
    setTimeout(() => {
      convertWikiLinksAsync(editor, nodeType);
    }, 0);
  },

  addProseMirrorPlugins() {
    const nodeType = this.type;
    const editorRef = this.editor;
    const pluginKey = new PluginKey("wikiLinkTransform");
    return [
      new Plugin({
        key: pluginKey,
        // After setContent (external file reload), convert any plain-text [[title]]
        appendTransaction(transactions, _oldState, _newState) {
          const isSetContent = transactions.some(
            (tr) => tr.docChanged && tr.getMeta("addToHistory") === false,
          );
          if (!isSetContent) return null;
          // Kick off async Rust extraction — can't return a tr synchronously
          convertWikiLinksAsync(editorRef, nodeType);
          return null;
        },
      }),
    ];
  },

  addStorage() {
    return {
      markdown: {
        serialize(state: any, node: any) {
          state.write(`[[${node.attrs.title}]]`);
        },
        parse: {
          setup(md: any) {
            wikiLinkPlugin(md);
          },
        },
      },
    };
  },
});

/** Collect text nodes from the PM doc and send them to Rust for wiki-link extraction. */
async function convertWikiLinksAsync(editor: any, nodeType: NodeType) {
  const nodes: TextNode[] = [];
  editor.state.doc.descendants((node: any, pos: number) => {
    if (!node.isText || !node.text) return;
    if (node.text.indexOf("[[") === -1) return;
    nodes.push({ text: node.text, pos });
  });
  if (nodes.length === 0) return;

  try {
    const replacements = await extractWikiLinks(nodes);
    if (replacements.length === 0) return;
    // Verify the editor is still alive and doc hasn't changed
    if (!editor.view || editor.isDestroyed) return;
    const tr = editor.state.tr;
    // Apply end→start so earlier positions aren't shifted
    for (let i = replacements.length - 1; i >= 0; i--) {
      const { from, to, title } = replacements[i];
      tr.replaceWith(from, to, nodeType.create({ title }));
    }
    tr.setMeta("addToHistory", false);
    editor.view.dispatch(tr);
  } catch {
    // IPC failed — silently degrade, links stay as plain text
  }
}

function wikiLinkPlugin(md: any) {
  md.inline.ruler.push("wiki_link", function (state: any, silent: boolean) {
    const src = state.src;
    const pos = state.pos;
    const max = state.posMax;

    if (pos + 3 > max) return false;
    if (pos > 0 && src.charCodeAt(pos - 1) === 0x21) return false; // skip ![[
    if (src.charCodeAt(pos) !== 0x5b) return false; // [
    if (src.charCodeAt(pos + 1) !== 0x5b) return false; // [

    const closePos = src.indexOf("]]", pos + 2);
    if (closePos === -1 || closePos >= max) return false;

    const title = src.slice(pos + 2, closePos).trim();
    if (!title) return false;

    if (silent) return true;

    const token = state.push("wiki_link", "span", 0);
    token.attrPush(["data-wiki-link", ""]);
    token.attrPush(["data-title", title]);
    token.attrPush(["class", "wiki-link"]);
    token.content = title;
    state.pos = closePos + 2;
    return true;
  });

  md.renderer.rules["wiki_link"] = function (tokens: any[], idx: number) {
    const token = tokens[idx];
    const title = token.attrGet("data-title") ?? "";
    const escaped = md.utils.escapeHtml(title);
    return `<span data-wiki-link="" data-title="${escaped}" class="wiki-link">[[${escaped}]]</span>`;
  };
}

export default WikiLink;
