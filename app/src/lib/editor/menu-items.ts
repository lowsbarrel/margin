import type { SlashMenuItem } from "./slash-command";

const items: SlashMenuItem[] = [
  {
    title: "Text",
    description: "Plain text paragraph.",
    icon: "¶",
    searchTerms: ["p", "paragraph", "text"],
    command: ({ editor, range }) => {
      editor
        .chain()
        .focus()
        .deleteRange(range)
        .toggleNode("paragraph", "paragraph")
        .run();
    },
  },
  {
    title: "Heading 1",
    description: "Large section heading.",
    icon: "H1",
    searchTerms: ["title", "big", "large", "h1"],
    command: ({ editor, range }) => {
      editor
        .chain()
        .focus()
        .deleteRange(range)
        .setNode("heading", { level: 1 })
        .run();
    },
  },
  {
    title: "Heading 2",
    description: "Medium section heading.",
    icon: "H2",
    searchTerms: ["subtitle", "medium", "h2"],
    command: ({ editor, range }) => {
      editor
        .chain()
        .focus()
        .deleteRange(range)
        .setNode("heading", { level: 2 })
        .run();
    },
  },
  {
    title: "Heading 3",
    description: "Small section heading.",
    icon: "H3",
    searchTerms: ["subtitle", "small", "h3"],
    command: ({ editor, range }) => {
      editor
        .chain()
        .focus()
        .deleteRange(range)
        .setNode("heading", { level: 3 })
        .run();
    },
  },
  {
    title: "To-do list",
    description: "Track tasks with checkboxes.",
    icon: "☑",
    searchTerms: ["todo", "task", "list", "check", "checkbox"],
    command: ({ editor, range }) => {
      editor.chain().focus().deleteRange(range).toggleTaskList().run();
    },
  },
  {
    title: "Bullet list",
    description: "Simple bullet list.",
    icon: "•",
    searchTerms: ["unordered", "point", "list", "ul"],
    command: ({ editor, range }) => {
      editor.chain().focus().deleteRange(range).toggleBulletList().run();
    },
  },
  {
    title: "Numbered list",
    description: "List with numbering.",
    icon: "1.",
    searchTerms: ["numbered", "ordered", "list", "ol"],
    command: ({ editor, range }) => {
      editor.chain().focus().deleteRange(range).toggleOrderedList().run();
    },
  },
  {
    title: "Quote",
    description: "Block quote.",
    icon: '"',
    searchTerms: ["blockquote", "quotes"],
    command: ({ editor, range }) => {
      editor.chain().focus().deleteRange(range).toggleBlockquote().run();
    },
  },
  {
    title: "Code",
    description: "Code block with syntax highlighting.",
    icon: "</>",
    searchTerms: ["codeblock", "code", "snippet"],
    command: ({ editor, range }) => {
      editor.chain().focus().deleteRange(range).toggleCodeBlock().run();
    },
  },
  {
    title: "Divider",
    description: "Horizontal rule divider.",
    icon: "—",
    searchTerms: ["horizontal rule", "hr", "divider", "separator"],
    command: ({ editor, range }) => {
      editor.chain().focus().deleteRange(range).setHorizontalRule().run();
    },
  },
  {
    title: "Table",
    description: "Insert a table.",
    icon: "▦",
    searchTerms: ["table", "rows", "columns", "grid"],
    command: ({ editor, range }) => {
      editor
        .chain()
        .focus()
        .deleteRange(range)
        .insertTable({ rows: 3, cols: 3, withHeaderRow: true })
        .run();
    },
  },
  {
    title: "Date",
    description: "Insert current date.",
    icon: "📅",
    searchTerms: ["date", "today", "time"],
    command: ({ editor, range }) => {
      const currentDate = new Date().toLocaleDateString("en-US", {
        year: "numeric",
        month: "long",
        day: "numeric",
      });
      editor
        .chain()
        .focus()
        .deleteRange(range)
        .insertContent(currentDate)
        .run();
    },
  },
  {
    title: "Callout",
    description: "Highlighted callout block.",
    icon: "💡",
    searchTerms: ["callout", "admonition", "alert", "info", "warning", "note"],
    command: ({ editor, range }) => {
      editor
        .chain()
        .focus()
        .deleteRange(range)
        .toggleCallout({ type: "info" })
        .run();
    },
  },
  {
    title: "Math Block",
    description: "Display math equation.",
    icon: "∑",
    searchTerms: ["math", "equation", "formula", "latex", "katex", "block"],
    command: ({ editor, range }) => {
      editor.chain().focus().deleteRange(range).setMathBlock().run();
    },
  },
  {
    title: "Inline Math",
    description: "Inline math expression.",
    icon: "π",
    searchTerms: ["math", "equation", "inline", "formula", "latex", "katex"],
    command: ({ editor, range }) => {
      editor.chain().focus().deleteRange(range).setMathInline().run();
    },
  },
];

export function getSlashMenuItems({
  query,
}: {
  query: string;
}): SlashMenuItem[] {
  if (!query) return items;
  const search = query.toLowerCase();
  return items.filter((item) => {
    return (
      item.title.toLowerCase().includes(search) ||
      item.description.toLowerCase().includes(search) ||
      item.searchTerms.some((term) => term.includes(search))
    );
  });
}

export default items;
