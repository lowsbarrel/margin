import { Node, mergeAttributes } from "@tiptap/core";

declare module "@tiptap/core" {
  interface Commands<ReturnType> {
    status: {
      setStatus: (attrs: { text?: string; color?: string }) => ReturnType;
    };
  }
}

const COLORS = ["gray", "blue", "green", "yellow", "red", "purple"] as const;

export const Status = Node.create({
  name: "status",
  group: "inline",
  inline: true,
  atom: true,
  selectable: true,

  addAttributes() {
    return {
      text: { default: "Status" },
      color: { default: "gray" },
    };
  },

  parseHTML() {
    return [{ tag: 'span[data-type="status"]' }];
  },

  renderHTML({ HTMLAttributes }) {
    return [
      "span",
      mergeAttributes(HTMLAttributes, {
        "data-type": "status",
        "data-color": HTMLAttributes.color || "gray",
        "data-text": HTMLAttributes.text || "Status",
        class: "status-badge",
      }),
      HTMLAttributes.text || "Status",
    ];
  },

  addCommands() {
    return {
      setStatus:
        (attrs) =>
        ({ commands }) => {
          return commands.insertContent({
            type: this.name,
            attrs: {
              text: attrs.text ?? "Status",
              color: attrs.color ?? "gray",
            },
          });
        },
    };
  },

  addNodeView() {
    return ({ node, editor, getPos }) => {
      const dom = document.createElement("span");
      dom.contentEditable = "false";
      dom.className = `status-badge status-${node.attrs.color}`;
      dom.textContent = node.attrs.text;

      let popover: HTMLDivElement | null = null;

      function removePopover() {
        if (popover) {
          popover.remove();
          popover = null;
        }
        document.removeEventListener("mousedown", onOutsideClick);
      }

      function onOutsideClick(e: MouseEvent) {
        if (popover && !popover.contains(e.target as HTMLElement)) {
          removePopover();
        }
      }

      function updateAttrs(attrs: { text?: string; color?: string }) {
        if (typeof getPos !== "function") return;
        const pos = getPos();
        if (pos == null) return;
        editor.view.dispatch(
          editor.view.state.tr.setNodeMarkup(pos, undefined, {
            ...node.attrs,
            ...attrs,
          }),
        );
      }

      function showPopover() {
        if (popover) return;

        popover = document.createElement("div");
        popover.className = "status-popover";
        popover.contentEditable = "false";

        const input = document.createElement("input");
        input.type = "text";
        input.className = "status-popover-input";
        input.value = node.attrs.text;
        input.addEventListener("input", () => {
          updateAttrs({ text: input.value });
        });
        input.addEventListener("keydown", (e) => {
          if (e.key === "Enter") {
            e.preventDefault();
            removePopover();
          }
        });
        popover.appendChild(input);

        const swatches = document.createElement("div");
        swatches.className = "status-popover-swatches";
        for (const c of COLORS) {
          const btn = document.createElement("button");
          btn.type = "button";
          btn.className = `status-swatch status-swatch-${c}`;
          if (c === node.attrs.color) btn.classList.add("active");
          btn.dataset.color = c;
          btn.addEventListener("click", (e) => {
            e.stopPropagation();
            updateAttrs({ color: c });
            swatches
              .querySelectorAll(".status-swatch")
              .forEach((s) => s.classList.remove("active"));
            btn.classList.add("active");
          });
          swatches.appendChild(btn);
        }
        popover.appendChild(swatches);

        dom.style.position = "relative";
        dom.appendChild(popover);

        requestAnimationFrame(() => input.focus());
        document.addEventListener("mousedown", onOutsideClick);
      }

      dom.addEventListener("click", (e) => {
        if (!editor.isEditable) return;
        e.stopPropagation();
        showPopover();
      });

      return {
        dom,
        update(updatedNode) {
          if (updatedNode.type.name !== "status") return false;
          dom.className = `status-badge status-${updatedNode.attrs.color}`;
          dom.textContent = updatedNode.attrs.text;
          if (popover) {
            dom.appendChild(popover);
            const input = popover.querySelector(
              ".status-popover-input",
            ) as HTMLInputElement | null;
            if (input && input !== document.activeElement) {
              input.value = updatedNode.attrs.text;
            }
            popover
              .querySelectorAll(".status-swatch")
              .forEach((s) =>
                s.classList.toggle(
                  "active",
                  (s as HTMLElement).dataset.color === updatedNode.attrs.color,
                ),
              );
          }
          return true;
        },
        destroy() {
          removePopover();
        },
      };
    };
  },
});
