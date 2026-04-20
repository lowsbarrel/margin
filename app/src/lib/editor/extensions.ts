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
import { Extension, type Extensions } from "@tiptap/core";
import { Plugin, PluginKey } from "@tiptap/pm/state";

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
    TaskItem.extend({
      addKeyboardShortcuts() {
        return {
          ...this.parent?.(),
          Backspace: ({ editor }) => {
            const { state } = editor;
            const { selection } = state;
            if (!selection.empty) return false;

            const { $from } = selection;
            // Only act at the very start of the node
            if ($from.parentOffset !== 0) return false;

            // Check we're in a taskItem
            let taskItemDepth: number | null = null;
            for (let d = $from.depth; d > 0; d--) {
              if ($from.node(d).type.name === "taskItem") {
                taskItemDepth = d;
                break;
              }
            }
            if (taskItemDepth === null) return false;

            // Check if taskList is nested inside a listItem
            const taskListDepth = taskItemDepth - 1;
            if (taskListDepth < 1) return false;
            const taskListNode = $from.node(taskListDepth);
            if (taskListNode.type.name !== "taskList") return false;

            const parentDepth = taskListDepth - 1;
            if (parentDepth < 1) return false;
            const parentNode = $from.node(parentDepth);
            if (parentNode.type.name !== "listItem") return false;

            const taskItemNode = $from.node(taskItemDepth);

            // Empty task item → delete it (and the taskList if last)
            if (taskItemNode.content.size <= 2) {
              if (taskListNode.childCount === 1) {
                // Last task item: remove the whole taskList
                const taskListPos = $from.before(taskListDepth);
                const tr = state.tr.delete(
                  taskListPos,
                  taskListPos + taskListNode.nodeSize,
                );
                editor.view.dispatch(tr);
              } else {
                // Remove just this task item
                const taskItemPos = $from.before(taskItemDepth);
                const tr = state.tr.delete(
                  taskItemPos,
                  taskItemPos + taskItemNode.nodeSize,
                );
                editor.view.dispatch(tr);
              }
              return true;
            }

            // Non-empty task item → lift out of task list into parent
            return editor.commands.liftListItem("taskItem");
          },
        };
      },
    }).configure({
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
      html: true,
      transformPastedText: true,
      transformCopiedText: false,
    }),
    // Selective clipboard serializer: only convert lists to markdown on copy.
    // Tables and other content use the default ProseMirror text serializer
    // (matching Docmost behaviour).
    Extension.create({
      name: "selectiveClipboardMarkdown",
      priority: 100,
      addProseMirrorPlugins() {
        return [
          new Plugin({
            key: new PluginKey("selectiveClipboardMarkdown"),
            props: {
              clipboardTextSerializer: (slice) => {
                const listTypes = ["bulletList", "orderedList", "taskList"];
                let topLevelCount = 0;
                let hasList = false;
                slice.content.forEach((node) => {
                  if (listTypes.includes(node.type.name)) {
                    hasList = true;
                    topLevelCount += node.childCount;
                  } else {
                    topLevelCount++;
                  }
                });
                if (!hasList || topLevelCount < 2) return null;
                const serializer =
                  (this.editor.storage as any).markdown?.serializer;
                if (!serializer) return null;
                return serializer.serialize(slice.content);
              },
            },
          }),
        ];
      },
    }),
    SearchReplace,
    ContentDrag,
  ];
}
