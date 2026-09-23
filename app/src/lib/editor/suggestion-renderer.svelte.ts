import { computePosition, flip, offset, shift } from '@floating-ui/dom';
import { mount, unmount, type Component } from 'svelte';
import type { SuggestionKeyDownProps, SuggestionProps } from '@tiptap/suggestion';

export interface SuggestionMenuProps<TItem> {
	items: TItem[];
	selectedIndex: number;
	onselect: (item: TItem) => void;
	onhover: (index: number) => void;
}

export interface SuggestionMenuExports {
	scrollToSelected: () => void;
}

export interface SuggestionRenderer<TItem> {
	onStart: (props: SuggestionProps<TItem, TItem>) => void | Promise<void>;
	onUpdate: (props: SuggestionProps<TItem, TItem>) => void | Promise<void>;
	onKeyDown: (props: SuggestionKeyDownProps) => boolean;
	onExit: () => void;
}

export interface CreateSuggestionRendererOptions<TItem> {
	component: Component<SuggestionMenuProps<TItem>, SuggestionMenuExports>;
	getItems: (query: string) => TItem[] | Promise<TItem[]>;
	loadItems?: () => Promise<void>;
}

export function createSuggestionRenderer<TItem>(
	opts: CreateSuggestionRendererOptions<TItem>
): () => SuggestionRenderer<TItem> {
	const { component, getItems, loadItems } = opts;

	return () => {
		let wrapper: HTMLDivElement | null = null;
		let items = $state<TItem[]>([]);
		let selectedIndex = $state(0);
		let command: ((item: TItem) => void) | null = null;
		let instance: SuggestionMenuExports | null = null;
		// A refresh awaits IPC and the menu can close mid-flight; a stale generation must not mount it.
		let generation = 0;

		function updatePosition(clientRect: () => DOMRect | null): void {
			if (!wrapper) return;
			const rect = clientRect();
			if (!rect) return;
			const virtualEl = { getBoundingClientRect: () => rect };

			computePosition(virtualEl, wrapper, {
				placement: 'bottom-start',
				middleware: [offset(8), flip(), shift({ padding: 8 })]
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
				const gen = ++generation;
				await refresh(props);
				if (gen !== generation) return;

				wrapper = document.createElement('div');
				wrapper.style.position = 'absolute';
				wrapper.style.zIndex = '999';
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
						}
					}
				});

				if (props.clientRect) {
					updatePosition(props.clientRect);
				}
			},

			onUpdate: async (props) => {
				const gen = generation;
				await refresh(props);
				if (gen !== generation) return;

				if (props.clientRect) {
					updatePosition(props.clientRect);
				}
			},

			onKeyDown: (props) => {
				const { event } = props;

				if (event.key === 'ArrowUp') {
					if (items.length === 0) return false;
					selectedIndex = (selectedIndex - 1 + items.length) % items.length;
					instance?.scrollToSelected();
					return true;
				}

				if (event.key === 'ArrowDown') {
					if (items.length === 0) return false;
					selectedIndex = (selectedIndex + 1) % items.length;
					instance?.scrollToSelected();
					return true;
				}

				if (event.key === 'Enter') {
					const selected = items[selectedIndex];
					if (selected) {
						command?.(selected);
					}
					return true;
				}

				if (event.key === 'Escape') {
					return true;
				}

				return false;
			},

			onExit: () => {
				generation++;
				if (instance && wrapper) {
					unmount(instance);
					wrapper.remove();
					wrapper = null;
					instance = null;
				}
				items = [];
				command = null;
			}
		};
	};
}
