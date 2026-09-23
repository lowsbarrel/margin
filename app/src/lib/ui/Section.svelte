<script lang="ts">
	import { untrack } from 'svelte';
	import { ChevronRight } from '@lucide/svelte';
	import type { Component, Snippet } from 'svelte';
	import { cn } from '$lib/utils';

	interface Props {
		title?: string;
		icon?: Component<{ size?: number }>;
		children: Snippet;
		collapsible?: boolean;
		defaultOpen?: boolean;
	}

	let { title, icon: Icon, children, collapsible = false, defaultOpen = true }: Props = $props();

	let open = $state(untrack(() => defaultOpen));

	function toggle() {
		if (collapsible) open = !open;
	}
</script>

<section class="flex flex-col overflow-hidden rounded-lg border border-border bg-background">
	{#if title}
		<!-- svelte-ignore a11y_click_events_have_key_events -->
		<!-- svelte-ignore a11y_no_static_element_interactions -->
		<div
			class={cn(
				'flex items-center justify-between bg-surface-1 px-4 py-3',
				(!collapsible || open) && 'border-b border-border',
				collapsible &&
					'cursor-pointer transition-colors duration-120 ease-out select-none hover:bg-surface-2'
			)}
			onclick={toggle}
		>
			<h3 class="m-0 flex items-center gap-2 text-sm font-semibold tracking-normal text-foreground">
				{#if Icon}<span class="flex items-center text-subtle-foreground"><Icon size={14} /></span
					>{/if}
				{title}
			</h3>
			{#if collapsible}
				<span
					class={cn(
						'flex items-center text-subtle-foreground transition-transform duration-150 ease-out',
						open && 'rotate-90'
					)}
				>
					<ChevronRight size={14} />
				</span>
			{/if}
		</div>
	{/if}
	{#if !collapsible || open}
		<div class="flex flex-col gap-3 p-4">
			{@render children()}
		</div>
	{/if}
</section>
