import { getSlashMenuItems } from "./menu-items";
import type { SlashMenuItem } from "./slash-command";
import { computePosition, flip, offset, shift } from "@floating-ui/dom";
import { mount, unmount } from "svelte";
import SlashMenu from "$lib/components/SlashMenu.svelte";

export default function renderSlashMenu() {
  let wrapper: HTMLDivElement | null = null;
  let items = $state<SlashMenuItem[]>([]);
  let selectedIndex = $state(0);
  let command: ((item: SlashMenuItem) => void) | null = null;
  let component: Record<string, any> | null = null;

  function updatePosition(clientRect: () => DOMRect) {
    if (!wrapper) return;
    const rect = clientRect();
    const virtualEl = { getBoundingClientRect: () => rect };

    computePosition(virtualEl, wrapper, {
      placement: "bottom-start",
      middleware: [offset(8), flip(), shift({ padding: 8 })],
    }).then(({ x, y }) => {
      if (wrapper) {
        wrapper.style.left = `${x}px`;
        wrapper.style.top = `${y}px`;
      }
    });
  }

  return {
    onStart: (props: any) => {
      items = getSlashMenuItems({ query: props.query });
      selectedIndex = 0;
      command = (item: SlashMenuItem) => {
        props.command(item);
      };

      wrapper = document.createElement("div");
      wrapper.style.position = "absolute";
      wrapper.style.zIndex = "999";
      document.body.appendChild(wrapper);

      component = mount(SlashMenu, {
        target: wrapper,
        props: {
          get items() {
            return items;
          },
          get selectedIndex() {
            return selectedIndex;
          },
          onselect: (item: SlashMenuItem) => command?.(item),
          onhover: (index: number) => {
            selectedIndex = index;
          },
        },
      });

      if (props.clientRect) {
        updatePosition(props.clientRect);
      }
    },

    onUpdate: (props: any) => {
      items = getSlashMenuItems({ query: props.query });
      selectedIndex = 0;
      command = (item: SlashMenuItem) => {
        props.command(item);
      };

      if (props.clientRect) {
        updatePosition(props.clientRect);
      }
    },

    onKeyDown: (props: any) => {
      const { event } = props;

      if (event.key === "ArrowUp") {
        selectedIndex = (selectedIndex - 1 + items.length) % items.length;
        (component as any)?.scrollToSelected?.();
        return true;
      }

      if (event.key === "ArrowDown") {
        selectedIndex = (selectedIndex + 1) % items.length;
        (component as any)?.scrollToSelected?.();
        return true;
      }

      if (event.key === "Enter") {
        if (items[selectedIndex]) {
          command?.(items[selectedIndex]);
        }
        return true;
      }

      if (event.key === "Escape") {
        return true;
      }

      return false;
    },

    onExit: () => {
      if (component && wrapper) {
        unmount(component);
        wrapper.remove();
        wrapper = null;
        component = null;
      }
      items = [];
      command = null;
    },
  };
}
