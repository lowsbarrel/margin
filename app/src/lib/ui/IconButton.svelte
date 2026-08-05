<script lang="ts">
	import type { Component } from 'svelte';
	import { cn } from '$lib/utils';

	interface Props {
		/* Lucide-compatible icon — see the note in Button.svelte for why the type
		   is the passed prop rather than `LucideProps`. */
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

<!--
  `p-0!` and `rounded-sm!` fight the unlayered `button { padding; border-radius }`
  block in app.css, which outranks plain utilities regardless of specificity.

  `[&.spin]:animate-spin` keeps the one caller-supplied class that carried
  behaviour (StatusBar passes extraClass="spin" while syncing) working without a
  scoped stylesheet — the class name comes from the call site, so it cannot be a
  utility, but a self-referencing arbitrary variant can match it.
-->
<button
	class={cn(
		'inline-flex shrink-0 items-center justify-center rounded-sm bg-transparent p-0 text-subtle-foreground transition-colors duration-120 ease-out',
		'hover:bg-muted hover:text-foreground',
		'disabled:pointer-events-none disabled:opacity-40',
		'[&.spin]:animate-spin',
		size === 'sm' ? 'size-[26px]' : 'size-8',
		active && 'bg-accent text-accent-foreground',
		extraClass
	)}
	{onclick}
	{title}
	{disabled}
>
	<Icon size={iconSize} />
</button>
