<script lang="ts">
	import type { Component, Snippet } from 'svelte';
	import { Button as ShadButton } from '$lib/components/ui/button/index.js';
	import { cn } from '$lib/utils';

	type Variant = 'primary' | 'secondary' | 'ghost' | 'brand' | 'danger' | 'success';
	type Size = 'sm' | 'md' | 'lg';

	interface Props {
		variant?: Variant;
		size?: Size;
		disabled?: boolean;
		loading?: boolean;
		icon?: Component<{ size?: number }>;
		onclick?: (e: MouseEvent) => void;
		children: Snippet;
		type?: 'button' | 'submit';
		title?: string;
		fullWidth?: boolean;
	}

	let {
		variant = 'secondary',
		size = 'md',
		disabled = false,
		loading = false,
		icon: Icon,
		onclick,
		children,
		type = 'button',
		title,
		fullWidth = false
	}: Props = $props();

	let iconSize = $derived(size === 'sm' ? 13 : size === 'lg' ? 16 : 14);

	const BASE = 'cursor-pointer gap-1.5 border py-0 disabled:cursor-not-allowed disabled:opacity-40';

	const SIZES: Record<Size, string> = {
		sm: "h-7 px-2.5 text-xs rounded-sm [&_svg:not([class*='size-'])]:size-[13px]",
		md: "h-8.5 px-3.5 text-sm rounded-sm [&_svg:not([class*='size-'])]:size-[14px]",
		lg: "h-10 px-4.5 text-sm rounded-md [&_svg:not([class*='size-'])]:size-4"
	};

	const VARIANTS: Record<Variant, string> = {
		primary: 'border-transparent bg-primary text-primary-foreground hover:opacity-[0.86]',
		secondary: 'border-input bg-background text-foreground hover:bg-muted',
		ghost:
			'border-transparent bg-transparent text-muted-foreground hover:bg-muted hover:text-foreground',
		brand: 'border-transparent bg-brand text-brand-foreground hover:bg-(--color-bg-brand-hover)',
		danger:
			'border-transparent bg-destructive text-destructive-foreground hover:bg-(--color-bg-negative-hover)',
		success: 'border-transparent bg-positive text-white hover:opacity-[0.86]'
	};
</script>

<ShadButton
	class={cn(BASE, SIZES[size], VARIANTS[variant], fullWidth && 'w-full')}
	{disabled}
	{type}
	{title}
	onclick={loading ? undefined : onclick}
>
	{#if loading}
		<span
			class="size-3.25 shrink-0 animate-spin rounded-full border-[1.5px] border-current border-t-transparent animation-duration-[0.6s]"
		></span>
	{:else if Icon}
		<Icon size={iconSize} />
	{/if}
	{@render children()}
</ShadButton>
