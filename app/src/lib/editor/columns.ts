import { Node, mergeAttributes } from "@tiptap/core";
import { NodeView } from "@tiptap/core";

declare module "@tiptap/core" {
  interface Commands<ReturnType> {
    columns: {
      insertColumns: (opts: { cols: number }) => ReturnType;
    };
  }
}

export const Column = Node.create({
  name: "column",
  content: "block+",
  isolating: true,

  parseHTML() {
    return [{ tag: "div[data-column]" }];
  },

  renderHTML({ HTMLAttributes }) {
    return [
      "div",
      mergeAttributes(HTMLAttributes, { "data-column": "" }),
      0,
    ];
  },

  addKeyboardShortcuts() {
    return {
      Tab: ({ editor }) => {
        const { $anchor } = editor.state.selection;
        const column = $anchor.node(-1);
        const columns = $anchor.node(-2);
        if (column?.type.name !== this.name || columns?.type.name !== "columns") {
          return false;
        }
        const colIndex = $anchor.index(-2);
        const colCount = columns.childCount;
        if (colIndex < colCount - 1) {
          const posAfterColumns = $anchor.before(-2);
          let offset = 0;
          for (let i = 0; i <= colIndex; i++) {
            offset += columns.child(i).nodeSize;
          }
          // +1 to enter the next column, +1 to enter its first child
          const targetPos = posAfterColumns + 1 + offset + 1;
          editor.commands.setTextSelection(targetPos);
          return true;
        }
        return false;
      },
      "Shift-Tab": ({ editor }) => {
        const { $anchor } = editor.state.selection;
        const column = $anchor.node(-1);
        const columns = $anchor.node(-2);
        if (column?.type.name !== this.name || columns?.type.name !== "columns") {
          return false;
        }
        const colIndex = $anchor.index(-2);
        if (colIndex > 0) {
          const posAfterColumns = $anchor.before(-2);
          let offset = 0;
          for (let i = 0; i < colIndex - 1; i++) {
            offset += columns.child(i).nodeSize;
          }
          const targetPos = posAfterColumns + 1 + offset + 1;
          editor.commands.setTextSelection(targetPos);
          return true;
        }
        return false;
      },
      Backspace: ({ editor }) => {
        const { $anchor, empty } = editor.state.selection;
        if (!empty) return false;

        const column = $anchor.node(-1);
        const columns = $anchor.node(-2);
        if (column?.type.name !== this.name || columns?.type.name !== "columns") {
          return false;
        }

        const colIndex = $anchor.index(-2);
        if (colIndex !== 0) return false;

        // Check cursor is at start of first child of first column
        const firstChild = column.firstChild;
        if (
          firstChild &&
          firstChild.type.name === "paragraph" &&
          firstChild.content.size === 0 &&
          $anchor.parentOffset === 0 &&
          $anchor.index(-1) === 0
        ) {
          // Unwrap: replace columns with content of all columns
          const { tr } = editor.state;
          const columnsPos = $anchor.before(-2);
          const content: any[] = [];
          columns.forEach((col) => {
            col.forEach((child) => {
              content.push(child);
            });
          });
          const { Fragment } = require("@tiptap/pm/model");
          tr.replaceWith(
            columnsPos,
            columnsPos + columns.nodeSize,
            Fragment.from(content),
          );
          editor.view.dispatch(tr);
          return true;
        }
        return false;
      },
    };
  },
});

const LAYOUTS: Record<string, number> = {
  two_equal: 2,
  three_equal: 3,
};

export const Columns = Node.create({
  name: "columns",
  group: "block",
  content: "column{2,5}",
  isolating: true,
  defining: true,

  addAttributes() {
    return {
      layout: {
        default: "two_equal",
        parseHTML: (el) => el.getAttribute("data-layout") || "two_equal",
        renderHTML: (attrs) => ({ "data-layout": attrs.layout }),
      },
    };
  },

  parseHTML() {
    return [{ tag: "div[data-columns]" }];
  },

  renderHTML({ HTMLAttributes, node }) {
    const cols = node.childCount;
    return [
      "div",
      mergeAttributes(HTMLAttributes, { "data-columns": String(cols) }),
      0,
    ];
  },

  addNodeView() {
    return ({ node, HTMLAttributes }) => {
      const dom = document.createElement("div");
      dom.classList.add("columns-block");
      dom.setAttribute("data-layout", node.attrs.layout);
      const contentDOM = document.createElement("div");
      contentDOM.classList.add("columns-content");
      dom.appendChild(contentDOM);
      return { dom, contentDOM };
    };
  },

  addCommands() {
    return {
      insertColumns:
        ({ cols }) =>
        ({ commands, state }) => {
          const layout =
            cols === 3 ? "three_equal" : "two_equal";
          const columnType = state.schema.nodes.column;
          const paragraphType = state.schema.nodes.paragraph;

          const columns = Array.from({ length: cols }, () =>
            columnType.createAndFill(null, paragraphType.create())!,
          );

          return commands.insertContent({
            type: this.name,
            attrs: { layout },
            content: columns.map((col) => ({
              type: "column",
              content: [{ type: "paragraph" }],
            })),
          });
        },
    };
  },
});
