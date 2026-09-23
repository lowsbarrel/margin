<script lang="ts">
	import type { Component } from 'svelte';
	import { cn } from '$lib/utils';

	interface Props {
		icon: Component<{ size?: number }>;
		onclick: (e: MouseEvent) => void;
		title?: string;
		size?: 'sm' | 'md';
		active?: boolean;
		disabled?: boolean;
		extraClass?: string;
	}

	let {
		icon: Icon,
		onclick,
		title,
		size = 'md',
		active = false,
		disabled = false,
		extraClass = ''
	}: Props = $props();
	let iconSize = $derived(size === 'sm' ? 14 : 16);
</script>

<button
	class={cn(
		'inline-flex shrink-0 items-center justify-center rounded-sm bg-transparent p-0 text-subtle-foreground transition-colors duration-120 ease-out',
		'hover:bg-muted hover:text-foreground',
		'disabled:pointer-events-none disabled:opacity-40',
		'[&.spin]:animate-spin',
		size === 'sm' ? 'size-6.5' : 'size-8',
		active && 'bg-accent text-accent-foreground',
		extraClass
	)}
	{onclick}
	{title}
	{disabled}
>
	<Icon size={iconSize} />
</button>
