import type { MentionItem } from "./mention-command";
import { computePosition, flip, offset, shift } from "@floating-ui/dom";
import { mount, unmount } from "svelte";
import MentionMenu from "$lib/components/MentionMenu.svelte";
import { vault } from "$lib/stores/vault.svelte";
import { searchFiles, walkDirectory, type FsEntry } from "$lib/fs/bridge";

/** Cache of all markdown files for fast filtering. Refreshed on each menu open. */
let cachedFiles: FsEntry[] = [];

async function loadAllFiles(): Promise<void> {
  if (!vault.vaultPath) return;
  cachedFiles = (await walkDirectory(vault.vaultPath)).filter(
    (e) => !e.is_dir && e.name.endsWith(".md"),
  );
}

function filterFiles(query: string): MentionItem[] {
  const q = query.toLowerCase();
  const scored: { item: MentionItem; score: number }[] = [];

  for (const entry of cachedFiles) {
    const name = entry.name.replace(/\.md$/, "");
    const nameLower = name.toLowerCase();

    let score = -1;
    if (!q) {
      // No query — show all, sorted alphabetically
      score = 0;
    } else {
      const idx = nameLower.indexOf(q);
      if (idx !== -1) {
        score = 1000 - idx + (q.length / nameLower.length) * 500;
      } else {
        // Fuzzy character match
        let qi = 0;
        let s = 0;
        let lastMatch = -1;
        for (let ti = 0; ti < nameLower.length && qi < q.length; ti++) {
          if (nameLower[ti] === q[qi]) {
            s += 10;
            if (lastMatch === ti - 1) s += 15;
            lastMatch = ti;
            qi++;
          }
        }
        if (qi === q.length) score = s;
      }
    }

    if (score >= 0) {
      scored.push({ item: { title: name, path: entry.path }, score });
    }
  }

  scored.sort(
    (a, b) => b.score - a.score || a.item.title.localeCompare(b.item.title),
  );
  return scored.slice(0, 20).map((s) => s.item);
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
      items = filterFiles(props.query);
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

    onUpdate: (props: any) => {
      items = filterFiles(props.query);
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
