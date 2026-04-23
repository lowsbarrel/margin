import { Node, nodeInputRule, mergeAttributes } from "@tiptap/core";
import type { Node as PMNode } from "@tiptap/pm/model";
import katex from "katex";
// Avoids re-rendering identical expressions (20-100ms each).
const katexCache = new Map<string, string>();
const KATEX_CACHE_MAX = 512;

function cachedKatex(text: string, displayMode: boolean): string {
  const key = `${displayMode ? "D" : "I"}:${text}`;
  const cached = katexCache.get(key);
  if (cached !== undefined) {
    // Move to end (most recently used) for LRU behavior
    katexCache.delete(key);
    katexCache.set(key, cached);
    return cached;
  }
  const html = katex.renderToString(text, { displayMode, throwOnError: false });
  if (katexCache.size >= KATEX_CACHE_MAX) {
    const first = katexCache.keys().next().value;
    if (first !== undefined) katexCache.delete(first);
  }
  katexCache.set(key, html);
  return html;
}
declare module "@tiptap/core" {
  interface Commands<ReturnType> {
    mathBlock: {
      setMathBlock: (attrs?: { text?: string }) => ReturnType;
    };
  }
}

export const mathBlockInputRegex = /(?:^|\s)((?:\$\$\$)((?:[^$]+))(?:\$\$\$))$/;

export const MathBlock = Node.create({
  name: "mathBlock",
  group: "block",
  atom: true,
  isolating: true,
  selectable: true,
  draggable: true,

  addAttributes() {
    return {
      text: {
        default: "",
        parseHTML: (el: HTMLElement) =>
          el.getAttribute("data-math-text") || el.textContent || "",
      },
    };
  },

  parseHTML() {
    return [
      {
        tag: 'div[data-type="mathBlock"]',
      },
    ];
  },

  renderHTML({ HTMLAttributes }) {
    const text = HTMLAttributes.text || "";
    let rendered: string;
    try {
      rendered = cachedKatex(text, true);
    } catch {
      rendered = `<code class="math-error">${escapeHtml(text)}</code>`;
    }

    return [
      "div",
      mergeAttributes({
        "data-type": "mathBlock",
        "data-math-text": text,
        class: "math-block",
        contenteditable: "false",
      }),
      ["div", { class: "math-render", innerHTML: rendered }],
    ];
  },

  addNodeView() {
    return ({ node, getPos, editor }) => {
      const dom = document.createElement("div");
      dom.classList.add("math-block");
      dom.setAttribute("data-type", "mathBlock");
      dom.contentEditable = "false";

      const renderArea = document.createElement("div");
      renderArea.classList.add("math-render");

      const inputArea = document.createElement("textarea");
      inputArea.classList.add("math-input");
      inputArea.value = node.attrs.text || "";
      inputArea.placeholder = "E = mc^2";
      inputArea.rows = 1;
      inputArea.style.display = "none";

      function renderMath(text: string) {
        try {
          renderArea.innerHTML = cachedKatex(text, true);
        } catch {
          renderArea.textContent = text || "Empty math block";
        }
        renderArea.classList.toggle("math-empty", !text);
      }

      renderMath(node.attrs.text || "");
      dom.addEventListener("click", (e) => {
        if (!editor.isEditable) return;
        e.stopPropagation();
        inputArea.style.display = "";
        renderArea.style.display = "none";
        inputArea.focus();
        autoResize(inputArea);
      });

      inputArea.addEventListener("input", () => {
        autoResize(inputArea);
      });

      inputArea.addEventListener("blur", () => {
        const newText = inputArea.value;
        inputArea.style.display = "none";
        renderArea.style.display = "";
        renderMath(newText);
        if (typeof getPos === "function") {
          const pos = getPos();
          if (pos != null) {
            editor.view.dispatch(
              editor.view.state.tr.setNodeMarkup(pos, undefined, {
                text: newText,
              }),
            );
          }
        }
      });

      inputArea.addEventListener("keydown", (e) => {
        if (e.key === "Escape" || (e.key === "Enter" && !e.shiftKey)) {
          e.preventDefault();
          inputArea.blur();
        }
      });

      dom.appendChild(renderArea);
      dom.appendChild(inputArea);

      return {
        dom,
        update(updatedNode: PMNode) {
          if (updatedNode.type.name !== "mathBlock") return false;
          if (inputArea.style.display === "none") {
            inputArea.value = updatedNode.attrs.text || "";
            renderMath(updatedNode.attrs.text || "");
          }
          return true;
        },
        stopEvent() {
          return true;
        },
        ignoreMutation() {
          return true;
        },
      };
    };
  },

  addCommands() {
    return {
      setMathBlock:
        (attrs) =>
        ({ commands }) =>
          commands.insertContent({
            type: "mathBlock",
            attrs: { text: attrs?.text ?? "" },
          }),
    };
  },

  addInputRules() {
    return [
      nodeInputRule({
        find: mathBlockInputRegex,
        type: this.type,
        getAttributes: (match) => ({
          text: match[2]?.trim() || "",
        }),
      }),
    ];
  },

  addStorage() {
    return {
      markdown: {
        serialize(state: any, node: PMNode) {
          state.write(`$$\n${node.attrs.text}\n$$`);
          state.closeBlock(node);
        },
        parse: {
          setup(md: any) {
            mathBlockMarkdownPlugin(md);
          },
        },
      },
    };
  },
});
declare module "@tiptap/core" {
  interface Commands<ReturnType> {
    mathInline: {
      setMathInline: (attrs?: { text?: string }) => ReturnType;
    };
  }
}

export const mathInlineInputRegex =
  /(?:^|\s)\$((?:[^$\s]|[^$\s][^$]*[^$\s]))\$$/;

export const MathInline = Node.create({
  name: "mathInline",
  group: "inline",
  inline: true,
  atom: true,
  selectable: true,

  addAttributes() {
    return {
      text: {
        default: "",
        parseHTML: (el: HTMLElement) =>
          el.getAttribute("data-math-text") || el.textContent || "",
      },
    };
  },

  parseHTML() {
    return [
      {
        tag: 'span[data-type="mathInline"]',
      },
    ];
  },

  renderHTML({ HTMLAttributes }) {
    const text = HTMLAttributes.text || "";
    let rendered: string;
    try {
      rendered = text
        ? cachedKatex(text, false)
        : '<span class="math-placeholder">math</span>';
    } catch {
      rendered = `<code class="math-error">${escapeHtml(text)}</code>`;
    }

    return [
      "span",
      mergeAttributes({
        "data-type": "mathInline",
        "data-math-text": text,
        class: "math-inline",
        contenteditable: "false",
      }),
      rendered,
    ];
  },

  addNodeView() {
    return ({ node, getPos, editor }) => {
      const dom = document.createElement("span");
      dom.classList.add("math-inline");
      dom.setAttribute("data-type", "mathInline");
      dom.contentEditable = "false";

      let currentText = node.attrs.text || "";

      function renderMath(text: string) {
        if (!text) {
          dom.innerHTML = '<span class="math-placeholder">math</span>';
          dom.classList.add("math-empty");
          return;
        }
        try {
          dom.innerHTML = cachedKatex(text, false);
        } catch {
          dom.textContent = text;
        }
        dom.classList.remove("math-empty");
      }

      renderMath(currentText);

      // If inserted empty (e.g. from slash menu), immediately open editor
      if (!currentText) {
        requestAnimationFrame(() => {
          if (!editor.isEditable) return;
          openEditor();
        });
      }

      function openEditor() {
        const input = document.createElement("input");
        input.type = "text";
        input.className = "math-inline-input";
        input.value = currentText;
        input.placeholder = "x^2 + y^2 = z^2";

        dom.innerHTML = "";
        dom.appendChild(input);
        input.focus();

        function commit() {
          const newText = input.value.trim();
          currentText = newText;

          if (typeof getPos === "function") {
            const pos = getPos();
            if (pos != null) {
              if (!newText) {
                editor.view.dispatch(
                  editor.view.state.tr.delete(pos, pos + node.nodeSize),
                );
                editor.view.focus();
                return;
              }
              editor.view.dispatch(
                editor.view.state.tr.setNodeMarkup(pos, undefined, {
                  text: newText,
                }),
              );
            }
          }
          renderMath(currentText);
          editor.view.focus();
        }

        let committed = false;
        input.addEventListener("blur", () => {
          if (!committed) {
            committed = true;
            commit();
          }
        });
        input.addEventListener("keydown", (e) => {
          if (e.key === "Enter" || e.key === "Escape") {
            e.preventDefault();
            e.stopPropagation();
            if (e.key === "Escape") {
              input.value = currentText;
            }
            committed = true;
            commit();
          }
        });
      }

      dom.addEventListener("click", (e) => {
        if (!editor.isEditable) return;
        e.stopPropagation();
        openEditor();
      });

      return {
        dom,
        update(updatedNode: PMNode) {
          if (updatedNode.type.name !== "mathInline") return false;
          currentText = updatedNode.attrs.text || "";
          renderMath(currentText);
          return true;
        },
        stopEvent() {
          return true;
        },
        ignoreMutation() {
          return true;
        },
      };
    };
  },

  addCommands() {
    return {
      setMathInline:
        (attrs) =>
        ({ commands }) =>
          commands.insertContent({
            type: "mathInline",
            attrs: { text: attrs?.text ?? "" },
          }),
    };
  },

  addInputRules() {
    return [
      nodeInputRule({
        find: mathInlineInputRegex,
        type: this.type,
        getAttributes: (match) => ({
          text: match[1]?.trim() || "",
        }),
      }),
    ];
  },

  addStorage() {
    return {
      markdown: {
        serialize(state: any, node: PMNode) {
          state.write(`$${node.attrs.text}$`);
        },
        parse: {
          setup(md: any) {
            mathInlineMarkdownPlugin(md);
          },
        },
      },
    };
  },
});
function mathBlockMarkdownPlugin(md: any) {
  md.block.ruler.before(
    "fence",
    "math_block",
    function (state: any, startLine: number, endLine: number, silent: boolean) {
      const pos = state.bMarks[startLine] + state.tShift[startLine];
      const max = state.eMarks[startLine];
      const src = state.src;

      if (pos + 2 > max) return false;
      if (src.charCodeAt(pos) !== 0x24 || src.charCodeAt(pos + 1) !== 0x24)
        return false;
      // Make sure it's just $$ (not $$$ which would be inline input rule territory)
      const rest = src.slice(pos + 2, max).trim();
      if (rest) return false;

      if (silent) return true;

      let nextLine = startLine + 1;
      let found = false;
      for (; nextLine < endLine; nextLine++) {
        const npos = state.bMarks[nextLine] + state.tShift[nextLine];
        const nmax = state.eMarks[nextLine];
        const line = src.slice(npos, nmax).trim();
        if (line === "$$") {
          found = true;
          break;
        }
      }
      if (!found) return false;
      const lines: string[] = [];
      for (let i = startLine + 1; i < nextLine; i++) {
        lines.push(
          src.slice(state.bMarks[i] + state.tShift[i], state.eMarks[i]),
        );
      }
      const mathText = lines.join("\n");

      const token = state.push("math_block", "div", 0);
      token.attrPush(["data-type", "mathBlock"]);
      token.attrPush(["data-math-text", mathText]);
      token.content = mathText;
      token.map = [startLine, nextLine + 1];

      state.line = nextLine + 1;
      return true;
    },
  );

  md.renderer.rules.math_block = function (tokens: any[], idx: number) {
    const token = tokens[idx];
    const text = token.attrGet("data-math-text") || token.content || "";
    const escaped = md.utils.escapeHtml(text);
    return `<div data-type="mathBlock" data-math-text="${escaped}">${escaped}</div>`;
  };
}

function mathInlineMarkdownPlugin(md: any) {
  md.inline.ruler.push("math_inline", function (state: any, silent: boolean) {
    const max = state.posMax;
    const src = state.src;
    const pos = state.pos;

    if (pos + 2 >= max) return false;
    if (src.charCodeAt(pos) !== 0x24) return false;

    // Single $ for inline math (not $$ which is for block)
    if (src.charCodeAt(pos + 1) === 0x24) return false;
    const closePos = src.indexOf("$", pos + 1);
    if (closePos === -1 || closePos > max) return false;
    const mathText = src.slice(pos + 1, closePos);
    if (!mathText.trim()) return false;

    if (silent) return true;

    const token = state.push("math_inline", "span", 0);
    token.attrPush(["data-type", "mathInline"]);
    token.attrPush(["data-math-text", mathText]);
    token.content = mathText;

    state.pos = closePos + 1;
    return true;
  });

  md.renderer.rules.math_inline = function (tokens: any[], idx: number) {
    const token = tokens[idx];
    const text = token.attrGet("data-math-text") || token.content || "";
    const escaped = md.utils.escapeHtml(text);
    return `<span data-type="mathInline" data-math-text="${escaped}">${escaped}</span>`;
  };
}
function escapeHtml(str: string): string {
  return str
    .replace(/&/g, "&amp;")
    .replace(/</g, "&lt;")
    .replace(/>/g, "&gt;")
    .replace(/"/g, "&quot;");
}

function autoResize(textarea: HTMLTextAreaElement) {
  textarea.style.height = "auto";
  textarea.style.height = textarea.scrollHeight + "px";
}
