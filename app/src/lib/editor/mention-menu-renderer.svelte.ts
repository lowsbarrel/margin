import type { MentionItem } from "./mention-command";
import { computePosition, flip, offset, shift } from "@floating-ui/dom";
import { mount, unmount } from "svelte";
import MentionMenu from "$lib/components/MentionMenu.svelte";
import { vault } from "$lib/stores/vault.svelte";
import { walkDirectory, type FsEntry } from "$lib/fs/bridge";
import { fuzzyFilterFiles, type FuzzyEntry } from "./text-transform-bridge";

/** Cache of all markdown files for fast filtering. Refreshed at most every 5 s. */
let cachedFiles: FuzzyEntry[] = [];
let cacheTimestamp = 0;
const CACHE_TTL = 5000; // 5 seconds

async function loadAllFiles(): Promise<void> {
  if (!vault.vaultPath) return;
  const now = Date.now();
  if (cachedFiles.length > 0 && now - cacheTimestamp < CACHE_TTL) return;
  const entries = await walkDirectory(vault.vaultPath);
  cachedFiles = entries
    .filter((e) => !e.is_dir && e.name.endsWith(".md"))
    .map((e) => ({ name: e.name, path: e.path }));
  cacheTimestamp = now;
}

async function filterFiles(query: string): Promise<MentionItem[]> {
  const results = await fuzzyFilterFiles(cachedFiles, query, 20);
  return results.map((r) => ({ title: r.name, path: r.path }));
}

export default function renderMentionMenu() {
  let wrapper: HTMLDivElement | null = null;
  let items = $state<MentionItem[]>([]);
  let selectedIndex = $state(0);
  let command: ((item: MentionItem) => void) | null = null;
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
    onStart: async (props: any) => {
      await loadAllFiles();
      items = await filterFiles(props.query);
      selectedIndex = 0;
      command = (item: MentionItem) => {
        props.command(item);
      };

      wrapper = document.createElement("div");
      wrapper.style.position = "absolute";
      wrapper.style.zIndex = "999";
      document.body.appendChild(wrapper);

      component = mount(MentionMenu, {
        target: wrapper,
        props: {
          get items() {
            return items;
          },
          get selectedIndex() {
            return selectedIndex;
          },
          onselect: (item: MentionItem) => command?.(item),
          onhover: (index: number) => {
            selectedIndex = index;
          },
        },
      });

      if (props.clientRect) {
        updatePosition(props.clientRect);
      }
    },

    onUpdate: async (props: any) => {
      items = await filterFiles(props.query);
      selectedIndex = 0;
      command = (item: MentionItem) => {
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
