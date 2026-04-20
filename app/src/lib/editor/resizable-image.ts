import Image from "@tiptap/extension-image";
import { mergeAttributes } from "@tiptap/core";
import type { Node as PMNode } from "@tiptap/pm/model";
import { NodeSelection, Plugin, PluginKey } from "@tiptap/pm/state";
import type { EditorView } from "@tiptap/pm/view";

declare module "@tiptap/core" {
  interface Commands<ReturnType> {
    resizableImage: {
      setImageAlign: (align: "left" | "center" | "right") => ReturnType;
      setImageWidth: (width: number) => ReturnType;
      setImageSize: (size: { width: number; height: number }) => ReturnType;
    };
  }
}

const MIN_WIDTH = 50;

export const ResizableImage = Image.extend({
  name: "image",

  addAttributes() {
    return {
      ...this.parent?.(),
      width: {
        default: null,
        parseHTML: (el: HTMLElement) => {
          const raw = el.getAttribute("width") || el.style.width;
          if (!raw) return null;
          const num = parseInt(raw, 10);
          return isNaN(num) ? null : num;
        },
        renderHTML: (attrs: Record<string, unknown>) => {
          if (!attrs.width) return {};
          return { width: attrs.width };
        },
      },
      height: {
        default: null,
        parseHTML: (el: HTMLElement) => {
          const raw = el.getAttribute("height") || el.style.height;
          if (!raw) return null;
          const num = parseInt(raw, 10);
          return isNaN(num) ? null : num;
        },
        renderHTML: (attrs: Record<string, unknown>) => {
          if (!attrs.height) return {};
          return { height: attrs.height };
        },
      },
      align: {
        default: "center",
        parseHTML: (el: HTMLElement) =>
          el.getAttribute("data-align") || "center",
        renderHTML: (attrs: Record<string, unknown>) => ({
          "data-align": attrs.align || "center",
        }),
      },
    };
  },

  parseHTML() {
    return [{ tag: "img[src]" }];
  },

  renderHTML({ HTMLAttributes }) {
    return [
      "img",
      mergeAttributes(this.options.HTMLAttributes, HTMLAttributes),
    ];
  },

  addCommands() {
    return {
      ...this.parent?.(),
      setImageAlign:
        (align) =>
        ({ commands }) =>
          commands.updateAttributes("image", { align }),
      setImageWidth:
        (width) =>
        ({ commands }) =>
          commands.updateAttributes("image", { width }),
      setImageSize:
        (size) =>
        ({ commands }) =>
          commands.updateAttributes("image", {
            width: size.width,
            height: size.height,
          }),
    };
  },

  addNodeView() {
    return ({ node, editor, getPos }) => {
      // --- DOM structure ---
      const container = document.createElement("div");
      container.classList.add("image-node-view");
      container.setAttribute("data-align", node.attrs.align || "center");

      const wrapper = document.createElement("div");
      wrapper.classList.add("image-node-view-wrapper");
      wrapper.style.position = "relative";
      wrapper.style.display = "inline-block";
      wrapper.style.lineHeight = "0";

      const img = document.createElement("img");
      img.src = node.attrs.src || "";
      img.alt = node.attrs.alt || "";
      img.draggable = false;
      if (node.attrs.width) {
        img.style.width = `${node.attrs.width}px`;
      }
      if (node.attrs.height) {
        img.style.height = `${node.attrs.height}px`;
      }

      wrapper.appendChild(img);

      // --- Resize handles ---
      const handleLeft = document.createElement("div");
      handleLeft.classList.add(
        "image-resize-handle",
        "image-resize-handle-left",
      );
      handleLeft.contentEditable = "false";

      const handleRight = document.createElement("div");
      handleRight.classList.add(
        "image-resize-handle",
        "image-resize-handle-right",
      );
      handleRight.contentEditable = "false";

      wrapper.appendChild(handleLeft);
      wrapper.appendChild(handleRight);

      // --- Alignment toolbar ---
      const toolbar = document.createElement("div");
      toolbar.classList.add("image-align-toolbar");
      toolbar.contentEditable = "false";
      toolbar.style.display = "none";

      const alignments: Array<{ value: string; icon: string }> = [
        {
          value: "left",
          icon: '<svg width="16" height="16" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2"><line x1="17" y1="10" x2="3" y2="10"/><line x1="21" y1="6" x2="3" y2="6"/><line x1="21" y1="14" x2="3" y2="14"/><line x1="17" y1="18" x2="3" y2="18"/></svg>',
        },
        {
          value: "center",
          icon: '<svg width="16" height="16" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2"><line x1="18" y1="10" x2="6" y2="10"/><line x1="21" y1="6" x2="3" y2="6"/><line x1="21" y1="14" x2="3" y2="14"/><line x1="18" y1="18" x2="6" y2="18"/></svg>',
        },
        {
          value: "right",
          icon: '<svg width="16" height="16" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2"><line x1="21" y1="10" x2="7" y2="10"/><line x1="21" y1="6" x2="3" y2="6"/><line x1="21" y1="14" x2="3" y2="14"/><line x1="21" y1="18" x2="7" y2="18"/></svg>',
        },
      ];

      for (const { value, icon } of alignments) {
        const btn = document.createElement("button");
        btn.type = "button";
        btn.classList.add("image-align-btn");
        btn.setAttribute("data-align", value);
        btn.innerHTML = icon;
        btn.title = `Align ${value}`;
        btn.addEventListener("mousedown", (e) => {
          e.preventDefault();
          e.stopPropagation();
          if (typeof getPos !== "function") return;
          const pos = getPos();
          if (pos == null) return;
          editor.chain().focus().setImageAlign(value as "left" | "center" | "right").run();
        });
        toolbar.appendChild(btn);
      }

      wrapper.appendChild(toolbar);
      container.appendChild(wrapper);

      // --- Selection tracking ---
      let selected = false;

      function updateToolbarVisibility() {
        toolbar.style.display = selected && editor.isEditable ? "" : "none";
        container.classList.toggle("image-selected", selected);

        // Update active button
        const currentAlign =
          container.getAttribute("data-align") || "center";
        toolbar
          .querySelectorAll<HTMLButtonElement>(".image-align-btn")
          .forEach((btn) => {
            btn.classList.toggle(
              "active",
              btn.getAttribute("data-align") === currentAlign,
            );
          });
      }

      // --- Resize logic ---
      let startX = 0;
      let startWidth = 0;
      let aspectRatio = 1;
      let resizing = false;
      let resizeDirection: "left" | "right" = "right";

      function onMouseDown(e: MouseEvent, direction: "left" | "right") {
        if (!editor.isEditable) return;
        e.preventDefault();
        e.stopPropagation();
        resizing = true;
        resizeDirection = direction;
        startX = e.clientX;
        startWidth = img.offsetWidth;
        aspectRatio =
          img.naturalWidth && img.naturalHeight
            ? img.naturalWidth / img.naturalHeight
            : startWidth / (img.offsetHeight || 1);

        container.classList.add("image-resizing");
        document.addEventListener("mousemove", onMouseMove);
        document.addEventListener("mouseup", onMouseUp);
      }

      function onMouseMove(e: MouseEvent) {
        if (!resizing) return;
        const containerEl = container.closest(".md-editor") as HTMLElement;
        const maxWidth = containerEl
          ? containerEl.clientWidth
          : window.innerWidth;

        let dx = e.clientX - startX;
        if (resizeDirection === "left") dx = -dx;

        let newWidth = Math.max(MIN_WIDTH, startWidth + dx);
        newWidth = Math.min(newWidth, maxWidth);
        const newHeight = Math.round(newWidth / aspectRatio);

        img.style.width = `${newWidth}px`;
        img.style.height = `${newHeight}px`;
      }

      function onMouseUp() {
        if (!resizing) return;
        resizing = false;
        container.classList.remove("image-resizing");
        document.removeEventListener("mousemove", onMouseMove);
        document.removeEventListener("mouseup", onMouseUp);

        const finalWidth = img.offsetWidth;
        const finalHeight = img.offsetHeight;

        if (typeof getPos !== "function") return;
        const pos = getPos();
        if (pos == null) return;

        editor.view.dispatch(
          editor.view.state.tr.setNodeMarkup(pos, undefined, {
            ...node.attrs,
            width: finalWidth,
            height: finalHeight,
          }),
        );
      }

      handleLeft.addEventListener("mousedown", (e) => onMouseDown(e, "left"));
      handleRight.addEventListener("mousedown", (e) =>
        onMouseDown(e, "right"),
      );

      // --- Selection plugin ---
      // Check selection on every editor transaction
      const selectionHandler = () => {
        const { selection } = editor.state;
        if (typeof getPos !== "function") return;
        const pos = getPos();
        if (pos == null) return;
        const isNodeSelection =
          selection instanceof NodeSelection && selection.from === pos;
        if (isNodeSelection !== selected) {
          selected = isNodeSelection;
          updateToolbarVisibility();
        }
      };

      editor.on("selectionUpdate", selectionHandler);
      editor.on("transaction", selectionHandler);

      return {
        dom: container,
        update: (updatedNode: PMNode) => {
          if (updatedNode.type.name !== "image") return false;
          // Use Object.assign to update the closed-over node reference
          // so that mouseup commits with up-to-date attrs
          (node as { attrs: typeof updatedNode.attrs }).attrs =
            updatedNode.attrs;

          img.src = updatedNode.attrs.src || "";
          img.alt = updatedNode.attrs.alt || "";
          container.setAttribute(
            "data-align",
            updatedNode.attrs.align || "center",
          );

          if (updatedNode.attrs.width && !resizing) {
            img.style.width = `${updatedNode.attrs.width}px`;
          }
          if (updatedNode.attrs.height && !resizing) {
            img.style.height = `${updatedNode.attrs.height}px`;
          }
          if (!updatedNode.attrs.width) {
            img.style.width = "";
          }
          if (!updatedNode.attrs.height) {
            img.style.height = "";
          }

          updateToolbarVisibility();
          return true;
        },
        selectNode: () => {
          selected = true;
          updateToolbarVisibility();
        },
        deselectNode: () => {
          selected = false;
          updateToolbarVisibility();
        },
        stopEvent: (event: Event) => {
          // Let the resize handles and toolbar buttons capture mouse events
          if (
            event.target instanceof HTMLElement &&
            (event.target.closest(".image-resize-handle") ||
              event.target.closest(".image-align-toolbar"))
          ) {
            return true;
          }
          return false;
        },
        ignoreMutation: () => true,
        destroy: () => {
          editor.off("selectionUpdate", selectionHandler);
          editor.off("transaction", selectionHandler);
          document.removeEventListener("mousemove", onMouseMove);
          document.removeEventListener("mouseup", onMouseUp);
        },
      };
    };
  },
});
