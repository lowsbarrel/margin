import { Node, mergeAttributes } from "@tiptap/core";
import { IMAGE_EXTS_ARRAY } from "$lib/utils/mime";

export interface FileEmbedOptions {
  attachmentFolder: string;
}

declare module "@tiptap/core" {
  interface Commands<ReturnType> {
    fileEmbed: {
      setFileEmbed: (options: { src: string; filename: string }) => ReturnType;
    };
  }
}

const FileEmbed = Node.create<FileEmbedOptions>({
  name: "fileEmbed",
  group: "block",
  atom: true,
  draggable: true,

  addOptions() {
    return {
      attachmentFolder: "attachments",
    };
  },

  addAttributes() {
    return {
      src: { default: null },
      filename: { default: null },
    };
  },

  parseHTML() {
    return [
      {
        tag: "div[data-file-embed]",
        getAttrs: (el) => {
          if (typeof el === "string") return false;
          return {
            src: el.getAttribute("data-src"),
            filename: el.getAttribute("data-filename"),
          };
        },
      },
    ];
  },

  renderHTML({ HTMLAttributes }) {
    const filename = HTMLAttributes.filename || "File";
    return [
      "div",
      mergeAttributes({
        "data-file-embed": "",
        "data-src": HTMLAttributes.src,
        "data-filename": filename,
        class: "file-embed",
        contenteditable: "false",
      }),
      ["span", { class: "file-embed-name" }, filename],
    ];
  },

  addCommands() {
    return {
      setFileEmbed:
        (options) =>
        ({ commands }) => {
          return commands.insertContent({
            type: this.name,
            attrs: options,
          });
        },
    };
  },

  addStorage() {
    return {
      markdown: {
        serialize(state: any, node: any) {
          state.write(`![[${node.attrs.filename}]]`);
          state.closeBlock(node);
        },
        parse: {
          setup(this: { options: FileEmbedOptions }, md: any) {
            const attachmentFolder = this.options.attachmentFolder;
            wikiFileEmbedPlugin(md, attachmentFolder);
          },
        },
      },
    };
  },
});

function wikiFileEmbedPlugin(md: any, attachmentFolder: string) {
  md.inline.ruler.push(
    "wiki_file_embed",
    function (state: any, silent: boolean) {
      const max = state.posMax;
      const src = state.src;
      const pos = state.pos;

      if (pos + 4 >= max) return false;
      if (src.charCodeAt(pos) !== 0x21) return false; // !
      if (src.charCodeAt(pos + 1) !== 0x5b) return false; // [
      if (src.charCodeAt(pos + 2) !== 0x5b) return false; // [

      const closePos = src.indexOf("]]", pos + 3);
      if (closePos === -1 || closePos > max) return false;

      const filename = src.slice(pos + 3, closePos);
      if (!filename || !filename.includes(".")) return false;

      const ext = filename.split(".").pop()?.toLowerCase() || "";
      // Images are handled by resolveWikiEmbeds preprocessing — skip them here
      if (IMAGE_EXTS_ARRAY.includes(ext)) return false;

      if (silent) return true;

      const token = state.push("file_embed", "div", 0);
      const fileSrc = filename.includes("/")
        ? filename
        : `${attachmentFolder}/${filename}`;
      token.attrPush(["data-file-embed", ""]);
      token.attrPush(["data-src", fileSrc]);
      token.attrPush(["data-filename", filename]);
      token.content = filename;

      state.pos = closePos + 2;
      return true;
    },
  );

  md.renderer.rules.file_embed = function (tokens: any[], idx: number) {
    const token = tokens[idx];
    const fileSrc = token.attrGet("data-src");
    const fileName = token.attrGet("data-filename");
    const eSrc = md.utils.escapeHtml(fileSrc);
    const eName = md.utils.escapeHtml(fileName);
    return `<div data-file-embed data-src="${eSrc}" data-filename="${eName}">${eName}</div>`;
  };
}

export default FileEmbed;
