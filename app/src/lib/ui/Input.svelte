<script lang="ts">
	import type { Component } from 'svelte';
	import { Input as ShadInput } from '$lib/components/ui/input/index.js';
	import { cn } from '$lib/utils';

	type Size = 'sm' | 'md' | 'lg';

	interface Props {
		value: string;
		onchange?: (value: string) => void;
		placeholder?: string;
		type?: 'text' | 'password' | 'email' | 'url';
		size?: Size;
		id?: string;
		/* Lucide-compatible icon — see the note in Button.svelte for why the type
		   is the passed prop rather than `LucideProps`. */
		icon?: Component<{ size?: number }>;
		disabled?: boolean;
		readonly?: boolean;
		mono?: boolean;
	}

	let {
		value = $bindable(),
		onchange,
		placeholder = '',
		type = 'text',
		size = 'md',
		id,
		icon: Icon,
		disabled = false,
		readonly = false,
		mono = false
	}: Props = $props();

	function handleInput(e: Event) {
		const target = e.target as HTMLInputElement;
		value = target.value;
		onchange?.(target.value);
	}

	/* app.css styles `input:not([type='checkbox'])` outside any cascade layer, so
	   it outranks utilities on background, border colour, padding, font-size and
	   radius — hence the `!` modifiers. It also supplies the focus ring
	   (`border-focus` + a 3px brand-16 glow) and the placeholder colour, which is
	   exactly what this component wants, so those are left alone.

	   `md:text-*!` is not redundant: shadcn's input carries `md:text-sm`, which
	   would otherwise win back the font size above the `md` breakpoint. */
	const BASE =
		'w-full bg-background text-foreground border-input disabled:bg-muted disabled:opacity-100 disabled:text-[var(--color-text-disabled)] disabled:cursor-not-allowed';

	const SIZES: Record<Size, string> = {
		sm: 'h-7 pl-2.5 pr-2.5 py-0 text-xs md:text-xs rounded-sm',
		md: 'h-[34px] pl-3 pr-3 py-0 text-sm md:text-sm rounded-sm',
		lg: 'h-10 pl-3.5 pr-3.5 py-0 text-sm md:text-sm rounded-md'
	};
</script>

<div class="relative flex w-full items-center">
	{#if Icon}
		<span class="pointer-events-none absolute left-3 flex items-center text-subtle-foreground">
			<Icon size={size === 'sm' ? 13 : 14} />
		</span>
	{/if}
	<ShadInput
		{type}
		{id}
		{placeholder}
		{disabled}
		{readonly}
		bind:value
		oninput={handleInput}
		class={cn(BASE, SIZES[size], mono && 'font-mono', Icon && 'pl-[34px]')}
	/>
</div>
