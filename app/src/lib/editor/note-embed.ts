import { Editor, Node, mergeAttributes, InputRule } from "@tiptap/core";
import type { Node as PMNode } from "@tiptap/pm/model";
import { vault } from "$lib/stores/vault.svelte";
import { searchFiles, readFileBytes } from "$lib/fs/bridge";
import { resolveImagePaths, resolveWikiEmbeds } from "$lib/editor/image-paths";
import { createEditorExtensions } from "$lib/editor/extensions";

/**
 * Minimal structural types for the tiptap-markdown serializer state and the
 * markdown-it inline plugin surface we touch. markdown-it@14 ships no bundled
 * .d.ts; we declare only the members used here.
 */
interface MarkdownSerializerState {
  write(content: string): void;
  closeBlock(node: PMNode): void;
}

interface MdToken {
  content: string;
  attrPush(attr: [string, string]): void;
  attrGet(name: string): string | null;
}

interface MdInlineState {
  src: string;
  pos: number;
  posMax: number;
  push(type: string, tag: string, nesting: number): MdToken;
}

type MdInlineRule = (state: MdInlineState, silent: boolean) => boolean;
type MdRenderRule = (tokens: MdToken[], idx: number) => string;

interface MarkdownIt {
  utils: { escapeHtml(str: string): string };
  inline: { ruler: { push(name: string, rule: MdInlineRule): void } };
  renderer: { rules: Record<string, MdRenderRule> };
}

export interface NoteEmbedOptions {
  attachmentFolder: string;
  lowlight: unknown;
  /** How deep this editor is nested inside transclusions (0 = top level). */
  depth: number;
}

/**
 * Live nested editors stop one level deep; deeper embeds render as a static
 * preview. This caps work and prevents A→B→A transclusion from recursing.
 */
const MAX_LIVE_DEPTH = 1;

/** `![[Foo]]` and `![[Foo.md]]` are notes; `![[file.pdf]]` is a file embed. */
function isNoteTarget(name: string): boolean {
  if (!name.includes(".")) return true;
  const ext = name.split(".").pop()?.toLowerCase();
  return ext === "md" || ext === "canvas";
}

function stripNoteExt(name: string): string {
  return name.replace(/\.(md|canvas)$/i, "");
}

declare module "@tiptap/core" {
  interface Commands<ReturnType> {
    noteEmbed: {
      setNoteEmbed: (title: string) => ReturnType;
    };
  }
}

const NoteEmbed = Node.create<NoteEmbedOptions>({
  name: "noteEmbed",
  group: "block",
  atom: true,
  draggable: true,
  selectable: true,

  addOptions() {
    return {
      attachmentFolder: "attachments",
      lowlight: null,
      depth: 0,
    };
  },

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
    return [{ tag: "div[data-note-embed]" }];
  },

  renderHTML({ node, HTMLAttributes }) {
    return [
      "div",
      mergeAttributes(
        { "data-note-embed": "", class: "note-embed", contenteditable: "false" },
        HTMLAttributes,
      ),
      `[[${node.attrs.title ?? ""}]]`,
    ];
  },

  addCommands() {
    return {
      setNoteEmbed:
        (title: string) =>
        ({ commands }) =>
          commands.insertContent({ type: this.name, attrs: { title } }),
    };
  },

  addInputRules() {
    return [
      new InputRule({
        find: /!\[\[([^\]\[\n]+)\]\]$/,
        handler: ({ range, match, chain }) => {
          const raw = match[1]?.trim();
          if (!raw || !isNoteTarget(raw)) return null;
          chain()
            .deleteRange(range)
            .insertContent({
              type: this.name,
              attrs: { title: stripNoteExt(raw) },
            })
            .run();
        },
      }),
    ];
  },

  addNodeView() {
    const options = this.options;
    return ({ node, editor }) => renderNoteEmbed(node, editor, options);
  },

  addStorage() {
    return {
      markdown: {
        serialize(state: MarkdownSerializerState, node: PMNode) {
          state.write(`![[${node.attrs.title}]]`);
          state.closeBlock(node);
        },
        parse: {
          setup(md: MarkdownIt) {
            noteEmbedMarkdownPlugin(md);
          },
        },
      },
    };
  },
});

function renderNoteEmbed(
  node: PMNode,
  editor: Editor,
  options: NoteEmbedOptions,
) {
  const title: string = node.attrs.title || "";

  const dom = document.createElement("div");
  dom.className = "note-embed";
  dom.setAttribute("data-note-embed", "");
  dom.setAttribute("data-title", title);
  dom.contentEditable = "false";

  // Header doubles as a wiki-link so the editor's existing click handler opens
  // the referenced note.
  const header = document.createElement("div");
  header.className = "note-embed-header";
  header.setAttribute("data-wiki-link", "");
  header.setAttribute("data-title", title);
  header.appendChild(
    Object.assign(document.createElement("span"), {
      className: "note-embed-icon",
    }),
  );
  header.appendChild(
    Object.assign(document.createElement("span"), {
      className: "note-embed-title",
      textContent: title,
    }),
  );

  const body = document.createElement("div");
  // `editor-wrap` opts the embedded content into the shared editor typography;
  // `.note-embed-body` overrides apply the embed-specific spacing.
  body.className = "note-embed-body editor-wrap";

  dom.appendChild(header);
  dom.appendChild(body);

  let nested: Editor | null = null;
  let destroyed = false;

  function showMessage(cls: string, text: string) {
    body.innerHTML = "";
    const el = document.createElement("div");
    el.className = cls;
    el.textContent = text;
    body.appendChild(el);
  }

  async function load() {
    if (!title) {
      showMessage("note-embed-empty", "Empty embed");
      return;
    }
    const vaultPath = vault.vaultPath;
    if (!vaultPath) return;
    try {
      const results = await searchFiles(vaultPath, title);
      const match = results.find((r) => !r.is_dir && r.name === `${title}.md`);
      if (!match) {
        showMessage("note-embed-missing", `Note not found: ${title}`);
        return;
      }
      const bytes = await readFileBytes(match.path);
      if (destroyed) return;
      let md = new TextDecoder().decode(bytes);
      md = resolveWikiEmbeds(md, options.attachmentFolder);
      md = resolveImagePaths(md, vaultPath);

      if (options.depth >= MAX_LIVE_DEPTH) {
        // Past the live-render cap: show static HTML, no further nested editors.
        const parser = (
          editor.storage as { markdown?: { parser?: { parse(s: string): string } } }
        ).markdown?.parser;
        body.innerHTML = parser ? parser.parse(md) : md;
        return;
      }

      nested = new Editor({
        element: body,
        editable: false,
        content: md,
        extensions: createEditorExtensions({
          lowlight: options.lowlight,
          attachmentFolder: options.attachmentFolder,
          embed: true,
          embedDepth: options.depth + 1,
        }),
        editorProps: { attributes: { class: "md-editor" } },
      });
    } catch {
      showMessage("note-embed-missing", `Could not load: ${title}`);
    }
  }

  load();

  return {
    dom,
    update(updatedNode: PMNode) {
      if (updatedNode.type.name !== "noteEmbed") return false;
      // Title changes are rare; let ProseMirror rebuild the node view.
      return updatedNode.attrs.title === title;
    },
    stopEvent() {
      return true;
    },
    ignoreMutation() {
      return true;
    },
    destroy() {
      destroyed = true;
      if (nested) {
        nested.destroy();
        nested = null;
      }
    },
  };
}

function noteEmbedMarkdownPlugin(md: MarkdownIt) {
  md.inline.ruler.push("note_embed", function (state, silent) {
    const src = state.src;
    const pos = state.pos;
    const max = state.posMax;

    if (pos + 4 >= max) return false;
    if (src.charCodeAt(pos) !== 0x21) return false; // !
    if (src.charCodeAt(pos + 1) !== 0x5b) return false; // [
    if (src.charCodeAt(pos + 2) !== 0x5b) return false; // [

    const closePos = src.indexOf("]]", pos + 3);
    if (closePos === -1 || closePos > max) return false;

    const raw = src.slice(pos + 3, closePos).trim();
    // Not a note target → let the file-embed rule claim it.
    if (!raw || !isNoteTarget(raw)) return false;

    if (silent) return true;

    const title = stripNoteExt(raw);
    const token = state.push("note_embed", "div", 0);
    token.attrPush(["data-note-embed", ""]);
    token.attrPush(["data-title", title]);
    token.content = title;
    state.pos = closePos + 2;
    return true;
  });

  md.renderer.rules.note_embed = function (tokens, idx) {
    const title = tokens[idx].attrGet("data-title") ?? "";
    const escaped = md.utils.escapeHtml(title);
    return `<div data-note-embed data-title="${escaped}">[[${escaped}]]</div>`;
  };
}

export default NoteEmbed;
