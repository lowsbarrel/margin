import { computePosition, flip, offset, shift } from "@floating-ui/dom";
import { mount, unmount, type Component } from "svelte";
import type { SuggestionKeyDownProps, SuggestionProps } from "@tiptap/suggestion";

/**
 * Props every suggestion menu component (SlashMenu/EmojiMenu/MentionMenu)
 * accepts. The factory mounts the component with these props.
 */
export interface SuggestionMenuProps<TItem> {
  items: TItem[];
  selectedIndex: number;
  onselect: (item: TItem) => void;
  onhover: (index: number) => void;
}

/**
 * Minimal interface exposed by the mounted menu components. Each menu
 * `export function scrollToSelected()`, which the factory calls on arrow nav.
 */
export interface SuggestionMenuExports {
  scrollToSelected: () => void;
}

/**
 * The object returned by a suggestion renderer, matching the shape that
 * `@tiptap/suggestion`'s `render()` callback must produce.
 */
export interface SuggestionRenderer<TItem> {
  onStart: (props: SuggestionProps<TItem, TItem>) => void | Promise<void>;
  onUpdate: (props: SuggestionProps<TItem, TItem>) => void | Promise<void>;
  onKeyDown: (props: SuggestionKeyDownProps) => boolean;
  onExit: () => void;
}

export interface CreateSuggestionRendererOptions<TItem> {
  /** The Svelte menu component to mount (SlashMenu/EmojiMenu/MentionMenu). */
  component: Component<SuggestionMenuProps<TItem>, SuggestionMenuExports>;
  /** Compute the items to show for the given query. May be async. */
  getItems: (query: string) => TItem[] | Promise<TItem[]>;
  /**
   * Optional hook run once before `getItems` on every start/update (used by
   * the mention variant to warm its cached file list).
   */
  loadItems?: () => Promise<void>;
}

/**
 * Builds a `@tiptap/suggestion` renderer factory. The returned function is the
 * `render` callback: invoking it produces a fresh {@link SuggestionRenderer}
 * that owns floating-ui positioning, mount/unmount, keyboard navigation and the
 * reactive `items`/`selectedIndex` state.
 */
export function createSuggestionRenderer<TItem>(
  opts: CreateSuggestionRendererOptions<TItem>,
): () => SuggestionRenderer<TItem> {
  const { component, getItems, loadItems } = opts;

  return () => {
    let wrapper: HTMLDivElement | null = null;
    let items = $state<TItem[]>([]);
    let selectedIndex = $state(0);
    let command: ((item: TItem) => void) | null = null;
    let instance: SuggestionMenuExports | null = null;

    function updatePosition(clientRect: () => DOMRect | null): void {
      if (!wrapper) return;
      const rect = clientRect();
      if (!rect) return;
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

    async function refresh(props: SuggestionProps<TItem, TItem>): Promise<void> {
      if (loadItems) await loadItems();
      items = await getItems(props.query);
      selectedIndex = 0;
      command = (item: TItem) => {
        props.command(item);
      };
    }

    return {
      onStart: async (props) => {
        await refresh(props);

        wrapper = document.createElement("div");
        wrapper.style.position = "absolute";
        wrapper.style.zIndex = "999";
        document.body.appendChild(wrapper);

        instance = mount(component, {
          target: wrapper,
          props: {
            get items() {
              return items;
            },
            get selectedIndex() {
              return selectedIndex;
            },
            onselect: (item: TItem) => command?.(item),
            onhover: (index: number) => {
              selectedIndex = index;
            },
          },
        });

        if (props.clientRect) {
          updatePosition(props.clientRect);
        }
      },

      onUpdate: async (props) => {
        await refresh(props);

        if (props.clientRect) {
          updatePosition(props.clientRect);
        }
      },

      onKeyDown: (props) => {
        const { event } = props;

        if (event.key === "ArrowUp") {
          if (items.length === 0) return false;
          selectedIndex = (selectedIndex - 1 + items.length) % items.length;
          instance?.scrollToSelected();
          return true;
        }

        if (event.key === "ArrowDown") {
          if (items.length === 0) return false;
          selectedIndex = (selectedIndex + 1) % items.length;
          instance?.scrollToSelected();
          return true;
        }

        if (event.key === "Enter") {
          const selected = items[selectedIndex];
          if (selected) {
            command?.(selected);
          }
          return true;
        }

        if (event.key === "Escape") {
          return true;
        }

        return false;
      },

      onExit: () => {
        if (instance && wrapper) {
          unmount(instance);
          wrapper.remove();
          wrapper = null;
          instance = null;
        }
        items = [];
        command = null;
      },
    };
  };
}
