import StarterKit from "@tiptap/starter-kit";
import { Table } from "@tiptap/extension-table";
import TableRow from "@tiptap/extension-table-row";
import TableCell from "@tiptap/extension-table-cell";
import TableHeader from "@tiptap/extension-table-header";
import TaskList from "@tiptap/extension-task-list";
import TaskItem from "@tiptap/extension-task-item";
import Placeholder from "@tiptap/extension-placeholder";
import Typography from "@tiptap/extension-typography";
import Highlight from "@tiptap/extension-highlight";
import markdownItMark from "markdown-it-mark";
import Link from "@tiptap/extension-link";
import Underline from "@tiptap/extension-underline";
import Superscript from "@tiptap/extension-superscript";
import Subscript from "@tiptap/extension-subscript";
import TextAlign from "@tiptap/extension-text-align";
import { TextStyle } from "@tiptap/extension-text-style";
import { Color } from "@tiptap/extension-color";
import CharacterCount from "@tiptap/extension-character-count";
import CodeBlockLowlight from "@tiptap/extension-code-block-lowlight";
import Image from "@tiptap/extension-image";
import { Markdown } from "tiptap-markdown";
import FileEmbed from "$lib/editor/file-embed";
import WikiLink from "$lib/editor/wiki-link";
import Callout from "$lib/editor/callout";
import { MathBlock, MathInline } from "$lib/editor/math";
import SlashCommand from "$lib/editor/slash-command";
import { getSlashMenuItems } from "$lib/editor/menu-items";
import renderSlashMenu from "$lib/editor/slash-menu-renderer.svelte";
import MentionCommand from "$lib/editor/mention-command";
import renderMentionMenu from "$lib/editor/mention-menu-renderer.svelte";
import { SearchReplace } from "$lib/editor/search-replace";
import { ContentDrag } from "$lib/editor/content-drag";
import type { Extensions } from "@tiptap/core";

interface CreateExtensionsOptions {
  lowlight: any;
  attachmentFolder?: string | null;
}

export function createEditorExtensions({
  lowlight,
  attachmentFolder,
}: CreateExtensionsOptions): Extensions {
  return [
    StarterKit.configure({
      heading: { levels: [1, 2, 3, 4, 5, 6] },
      codeBlock: false,
      link: false,
      underline: false,
      dropcursor: {
        width: 3,
        color: "#70CFF8",
      },
    }),
    CodeBlockLowlight.configure({
      lowlight,
      HTMLAttributes: {
        spellcheck: "false",
      },
    }),
    Table.configure({
      resizable: true,
      lastColumnResizable: true,
      allowTableNodeSelection: true,
    }),
    TableRow,
    TableCell,
    TableHeader,
    TaskList,
    TaskItem.configure({
      nested: true,
    }),
    Placeholder.configure({
      placeholder: ({ node }) => {
        if (node.type.name === "heading") {
          return `Heading ${node.attrs.level}`;
        }
        return 'Write anything, "/" for commands, "@" to link a note...';
      },
      includeChildren: true,
      showOnlyWhenEditable: true,
    }),
    Typography,
    Highlight.extend({
      addStorage() {
        return {
          markdown: {
            serialize: { open: "==", close: "==" },
            parse: {
              setup(md: any) {
                md.use(markdownItMark);
              },
              updateDOM(el: HTMLElement) {
                el.querySelectorAll("mark").forEach((mark: HTMLElement) => {
                  mark.setAttribute("data-color", "");
                });
              },
            },
          },
        };
      },
    }).configure({
      multicolor: true,
    }),
    Link.configure({
      openOnClick: false,
    }),
    Underline,
    Superscript,
    Subscript,
    TextAlign.configure({
      types: ["heading", "paragraph"],
    }),
    TextStyle,
    Color,
    CharacterCount,
    SlashCommand.configure({
      suggestion: {
        items: getSlashMenuItems,
        render: renderSlashMenu,
      },
    }),
    Image.configure({
      inline: false,
      allowBase64: true,
    }),
    FileEmbed.configure({
      attachmentFolder: attachmentFolder || "attachments",
    }),
    MentionCommand.configure({
      suggestion: {
        items: () => [{ id: "placeholder" }],
        render: renderMentionMenu,
      },
    }),
    WikiLink,
    Callout,
    MathBlock,
    MathInline,
    Markdown.configure({
      html: false,
      transformPastedText: true,
      transformCopiedText: true,
    }),
    SearchReplace,
    ContentDrag,
  ];
}
