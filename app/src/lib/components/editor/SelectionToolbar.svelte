<script lang="ts">
	import type { Component } from 'svelte';
	import type { EditorView } from '@codemirror/view';
	import {
		Bold,
		Check,
		ChevronDown,
		Code,
		Heading,
		Heading1,
		Heading2,
		Heading3,
		Highlighter,
		Italic,
		Link,
		List,
		ListOrdered,
		ListTodo,
		Quote,
		SquareCode,
		Strikethrough,
		Type,
		X
	} from '@lucide/svelte';
	import * as m from '$lib/paraglide/messages.js';
	import { cn } from '$lib/utils';
	import * as DropdownMenu from '$lib/components/ui/dropdown-menu/index.js';
	import * as Tooltip from '$lib/components/ui/tooltip/index.js';
	import { setBlock, setLink, toggleMark, type BlockType } from '$lib/editor/live/commands';
	import type { ToolbarState } from '$lib/editor/live/bubble-state.svelte';

	interface Props {
		view: EditorView;
		status: ToolbarState;
	}

	let { view, status }: Props = $props();

	type Icon = Component<{ size?: number }>;

	const TOGGLE =
		'flex size-7 shrink-0 cursor-pointer items-center justify-center rounded-xs p-0 text-muted-foreground transition-colors hover:bg-surface-3 hover:text-foreground';
	const ACTIVE = 'bg-accent text-accent-foreground hover:bg-accent hover:text-accent-foreground';
	const LINK_BTN =
		'flex size-6.5 shrink-0 cursor-pointer items-center justify-center rounded-xs p-0 transition-colors hover:bg-surface-3';

	const MARKS: { id: string; marker: string; label: () => string; icon: Icon }[] = [
		{ id: 'bold', marker: '**', label: m.bubble_bold, icon: Bold },
		{ id: 'italic', marker: '*', label: m.bubble_italic, icon: Italic },
		{ id: 'strike', marker: '~~', label: m.bubble_strike, icon: Strikethrough },
		{ id: 'code', marker: '`', label: m.bubble_code, icon: Code },
		{ id: 'highlight', marker: '==', label: m.bubble_highlight, icon: Highlighter }
	];

	const BLOCK_LABEL: Record<BlockType, () => string> = {
		text: m.bubble_text,
		heading1: () => m.bubble_heading({ level: 1 }),
		heading2: () => m.bubble_heading({ level: 2 }),
		heading3: () => m.bubble_heading({ level: 3 }),
		heading4: () => m.bubble_heading({ level: 4 }),
		heading5: () => m.bubble_heading({ level: 5 }),
		heading6: () => m.bubble_heading({ level: 6 }),
		bullet: m.bubble_block_bullet,
		ordered: m.bubble_block_ordered,
		task: m.bubble_block_task,
		quote: m.bubble_block_quote,
		code: m.bubble_block_code
	};

	const BLOCK_ICON: Record<BlockType, Icon> = {
		text: Type,
		heading1: Heading1,
		heading2: Heading2,
		heading3: Heading3,
		heading4: Heading,
		heading5: Heading,
		heading6: Heading,
		bullet: List,
		ordered: ListOrdered,
		task: ListTodo,
		quote: Quote,
		code: SquareCode
	};

	const MENU: BlockType[] = [
		'text',
		'heading1',
		'heading2',
		'heading3',
		'bullet',
		'ordered',
		'task',
		'quote',
		'code'
	];

	let rootEl: HTMLDivElement | undefined = $state();
	let linkInput: HTMLInputElement | undefined = $state();
	let linkOpen = $state(false);
	let url = $state('');
	const CurrentIcon = $derived(BLOCK_ICON[status.block]);
	const currentLabel = $derived(BLOCK_LABEL[status.block]());

	$effect(() => {
		if (linkOpen) linkInput?.focus();
	});

	function runMark(marker: string) {
		toggleMark(view, marker);
		view.focus();
	}

	function runBlock(type: BlockType) {
		setBlock(view, type);
		view.focus();
	}

	function closeLink() {
		linkOpen = false;
		url = '';
		view.focus();
	}

	function submitLink() {
		const target = url.trim();
		if (target) setLink(view, target);
		closeLink();
	}

	function handleLinkKeydown(event: KeyboardEvent) {
		if (event.key === 'Enter') {
			event.preventDefault();
			submitLink();
		} else if (event.key === 'Escape') {
			event.preventDefault();
			closeLink();
		}
	}
</script>

<Tooltip.Provider delayDuration={300}>
	<div
		class="surface-popover flex items-center gap-0.5 px-1.5 py-1 font-sans"
		bind:this={rootEl}
		role="toolbar"
	>
		{#if linkOpen}
			<div class="flex items-center gap-1">
				<input
					bind:this={linkInput}
					bind:value={url}
					type="url"
					placeholder="https://..."
					aria-label={m.bubble_link()}
					class="h-6.5 w-50 rounded-xs border border-border bg-surface-2 px-2 py-0 font-mono text-sm text-foreground caret-foreground outline-none placeholder:text-subtle-foreground"
					onkeydown={handleLinkKeydown}
				/>
				<button
					type="button"
					class={cn(LINK_BTN, 'text-muted-foreground hover:text-positive')}
					aria-label={m.bubble_confirm_link()}
					onclick={submitLink}
				>
					<Check size={16} />
				</button>
				<button
					type="button"
					class={cn(LINK_BTN, 'text-muted-foreground hover:text-destructive')}
					aria-label={m.bubble_cancel_link()}
					onclick={closeLink}
				>
					<X size={16} />
				</button>
			</div>
		{:else}
			<DropdownMenu.Root
				onOpenChangeComplete={(open) => {
					if (!open) view.focus();
				}}
			>
				<DropdownMenu.Trigger
					class={cn(TOGGLE, 'w-auto gap-1 px-1.5')}
					aria-label={m.bubble_block_type()}
					data-cmd="block"
				>
					<CurrentIcon size={16} />
					<span class="text-xs">{currentLabel}</span>
					<ChevronDown size={14} class="text-subtle-foreground" />
				</DropdownMenu.Trigger>
				<DropdownMenu.Content class="min-w-44 outline-hidden" portalProps={{ to: rootEl }}>
					{#each MENU as type (type)}
						{@const Icon = BLOCK_ICON[type]}
						<DropdownMenu.CheckboxItem
							checked={status.block === type}
							onSelect={() => runBlock(type)}
							data-cmd={type}
						>
							<Icon size={16} />
							<span>{BLOCK_LABEL[type]()}</span>
						</DropdownMenu.CheckboxItem>
					{/each}
				</DropdownMenu.Content>
			</DropdownMenu.Root>

			<span class="mx-0.75 h-4.5 w-px bg-border"></span>

			{#each MARKS as item (item.id)}
				{@const Icon = item.icon}
				<Tooltip.Root>
					<Tooltip.Trigger
						class={cn(TOGGLE, status.marks.includes(item.id) && ACTIVE)}
						aria-label={item.label()}
						data-cmd={item.id}
						onclick={() => runMark(item.marker)}
					>
						<Icon size={16} />
					</Tooltip.Trigger>
					<Tooltip.Content>{item.label()}</Tooltip.Content>
				</Tooltip.Root>
			{/each}

			<span class="mx-0.75 h-4.5 w-px bg-border"></span>

			<Tooltip.Root>
				<Tooltip.Trigger
					class={TOGGLE}
					aria-label={m.bubble_link()}
					data-cmd="link"
					onclick={() => (linkOpen = true)}
				>
					<Link size={16} />
				</Tooltip.Trigger>
				<Tooltip.Content>{m.bubble_link()}</Tooltip.Content>
			</Tooltip.Root>
		{/if}
	</div>
</Tooltip.Provider>
