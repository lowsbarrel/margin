import { Node, mergeAttributes } from "@tiptap/core";
import type { Node as PMNode } from "@tiptap/pm/model";
import type MermaidApi from "mermaid";

/**
 * Minimal structural type for the tiptap-markdown serializer state we touch.
 * markdown-it@14 ships no bundled .d.ts; we declare only what we use.
 */
interface MarkdownSerializerState {
  write(content: string): void;
  closeBlock(node: PMNode): void;
}

let renderSeq = 0;

// mermaid is a large dependency, so it is loaded on demand the first time a
// diagram renders rather than eagerly at editor (and app) startup.
let mermaidPromise: Promise<typeof MermaidApi> | null = null;

async function loadMermaid(): Promise<typeof MermaidApi> {
  if (!mermaidPromise) {
    mermaidPromise = import("mermaid").then((mod) => {
      const mermaid = mod.default;
      const light =
        document.documentElement.getAttribute("data-theme") === "light";
      mermaid.initialize({
        startOnLoad: false,
        theme: light ? "default" : "dark",
        securityLevel: "strict",
        fontFamily: "inherit",
      });
      return mermaid;
    });
  }
  return mermaidPromise;
}

/** Render `code` into `target` as an SVG diagram, swallowing render errors. */
async function renderInto(target: HTMLElement, code: string): Promise<void> {
  if (!code.trim()) {
    target.innerHTML = "";
    const empty = document.createElement("div");
    empty.className = "mermaid-empty";
    empty.textContent = "Empty diagram — click to edit";
    target.appendChild(empty);
    return;
  }
  const id = `mmd-${++renderSeq}`;
  try {
    const mermaid = await loadMermaid();
    // parse() validates without leaving an orphan DOM node on failure
    await mermaid.parse(code);
    const { svg } = await mermaid.render(id, code);
    target.innerHTML = svg;
  } catch (err) {
    target.innerHTML = "";
    const errEl = document.createElement("div");
    errEl.className = "mermaid-error";
    errEl.textContent = err instanceof Error ? err.message : String(err);
    target.appendChild(errEl);
  }
}

function autoResize(textarea: HTMLTextAreaElement) {
  textarea.style.height = "auto";
  textarea.style.height = `${textarea.scrollHeight}px`;
}

declare module "@tiptap/core" {
  interface Commands<ReturnType> {
    mermaid: {
      setMermaid: (attrs?: { code?: string }) => ReturnType;
    };
  }
}

export const Mermaid = Node.create({
  name: "mermaid",
  group: "block",
  atom: true,
  isolating: true,
  selectable: true,
  draggable: true,
  // Beat CodeBlockLowlight when parsing a ```mermaid fence from markdown.
  priority: 200,

  addAttributes() {
    return {
      code: {
        default: "",
        parseHTML: (el: HTMLElement) =>
          el.getAttribute("data-mermaid") ?? el.textContent ?? "",
      },
    };
  },

  parseHTML() {
    return [
      { tag: 'div[data-type="mermaid"]' },
      {
        // ```mermaid fences round-trip through markdown-it as
        // <pre><code class="language-mermaid">…</code></pre>
        tag: "pre",
        preserveWhitespace: "full",
        getAttrs: (el) => {
          if (typeof el === "string") return false;
          const code = el.querySelector("code");
          const cls = code?.getAttribute("class") || "";
          if (!/(^|\s)language-mermaid(\s|$)/.test(cls)) return false;
          return { code: (code?.textContent ?? "").replace(/\n$/, "") };
        },
      },
    ];
  },

  renderHTML({ HTMLAttributes }) {
    const code = HTMLAttributes.code || "";
    return [
      "div",
      mergeAttributes({
        "data-type": "mermaid",
        "data-mermaid": code,
        class: "mermaid-block",
        contenteditable: "false",
      }),
      code,
    ];
  },

  addNodeView() {
    return ({ node, getPos, editor }) => {
      const dom = document.createElement("div");
      dom.classList.add("mermaid-block");
      dom.setAttribute("data-type", "mermaid");
      dom.contentEditable = "false";

      const renderArea = document.createElement("div");
      renderArea.classList.add("mermaid-render");

      const input = document.createElement("textarea");
      input.classList.add("mermaid-input");
      input.value = node.attrs.code || "";
      input.placeholder = "graph TD;\n  A[Start] --> B[End];";
      input.rows = 3;
      input.spellcheck = false;
      input.style.display = "none";

      let editing = false;
      let lastRendered = node.attrs.code || "";

      renderInto(renderArea, lastRendered);

      function openEditor() {
        if (!editor.isEditable) return;
        editing = true;
        input.style.display = "";
        renderArea.style.display = "none";
        input.focus();
        autoResize(input);
      }

      // Inserted empty (e.g. from slash menu) → open editor on next frame.
      let openRafId: number | null = null;
      if (!node.attrs.code) {
        openRafId = requestAnimationFrame(() => {
          openRafId = null;
          if (typeof getPos !== "function" || getPos() == null) return;
          openEditor();
        });
      }

      dom.addEventListener("click", (e) => {
        if (editing || !editor.isEditable) return;
        e.stopPropagation();
        openEditor();
      });

      input.addEventListener("input", () => autoResize(input));

      input.addEventListener("blur", () => {
        editing = false;
        const code = input.value;
        input.style.display = "none";
        renderArea.style.display = "";
        lastRendered = code;
        renderInto(renderArea, code);
        if (typeof getPos === "function") {
          const pos = getPos();
          if (pos != null) {
            editor.view.dispatch(
              editor.view.state.tr.setNodeMarkup(pos, undefined, { code }),
            );
          }
        }
      });

      input.addEventListener("keydown", (e) => {
        if (e.key === "Escape" || (e.key === "Enter" && (e.ctrlKey || e.metaKey))) {
          e.preventDefault();
          input.blur();
        }
      });

      dom.appendChild(renderArea);
      dom.appendChild(input);

      return {
        dom,
        update(updatedNode: PMNode) {
          if (updatedNode.type.name !== "mermaid") return false;
          const code = updatedNode.attrs.code || "";
          if (!editing && code !== lastRendered) {
            input.value = code;
            lastRendered = code;
            renderInto(renderArea, code);
          }
          return true;
        },
        stopEvent() {
          return true;
        },
        ignoreMutation() {
          return true;
        },
        destroy() {
          if (openRafId != null) cancelAnimationFrame(openRafId);
        },
      };
    };
  },

  addCommands() {
    return {
      setMermaid:
        (attrs) =>
        ({ commands }) =>
          commands.insertContent({
            type: this.name,
            attrs: { code: attrs?.code ?? "" },
          }),
    };
  },

  addStorage() {
    return {
      markdown: {
        serialize(state: MarkdownSerializerState, node: PMNode) {
          state.write(`\`\`\`mermaid\n${node.attrs.code || ""}\n\`\`\``);
          state.closeBlock(node);
        },
        // No parse setup needed: the default markdown-it fence renderer emits
        // <pre><code class="language-mermaid">, which parseHTML() claims.
      },
    };
  },
});

export default Mermaid;
