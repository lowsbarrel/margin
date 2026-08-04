<script lang="ts">
	import { Textarea as ShadTextarea } from '$lib/components/ui/textarea/index.js';
	import { cn } from '$lib/utils';

	type Size = 'sm' | 'md' | 'lg';

	interface Props {
		value: string;
		onchange?: (value: string) => void;
		placeholder?: string;
		size?: Size;
		id?: string;
		disabled?: boolean;
		readonly?: boolean;
		mono?: boolean;
		rows?: number;
	}

	let {
		value = $bindable(),
		onchange,
		placeholder = '',
		size = 'md',
		id,
		disabled = false,
		readonly = false,
		mono = true,
		rows = 3
	}: Props = $props();

	function handleInput(e: Event) {
		const target = e.target as HTMLTextAreaElement;
		value = target.value;
		onchange?.(target.value);
	}

	/* `field-sizing-fixed` and `min-h-0` undo shadcn's auto-growing textarea:
	   this component sizes itself from the `rows` prop, and `field-sizing-content`
	   would silently ignore it.

	   The `!` modifiers win back the properties app.css claims on the bare
	   `textarea` element from outside any cascade layer (background, border
	   colour, padding, font-size/family, radius). Its focus ring and placeholder
	   colour already match the design, so they are left in place. */
	const BASE =
		'w-full resize-y field-sizing-fixed min-h-0 leading-5 bg-background text-foreground border-input disabled:bg-muted disabled:opacity-100 disabled:text-[var(--color-text-disabled)] disabled:cursor-not-allowed';

	const SIZES: Record<Size, string> = {
		sm: 'px-2.5 py-[7px] text-xs md:text-xs rounded-sm',
		md: 'px-3 py-[9px] text-sm md:text-sm rounded-sm',
		lg: 'px-3.5 py-[11px] text-sm md:text-sm rounded-md'
	};
</script>

<ShadTextarea
	{id}
	{placeholder}
	{disabled}
	{readonly}
	{rows}
	bind:value
	oninput={handleInput}
	class={cn(BASE, SIZES[size], mono ? 'font-mono' : 'font-sans')}
/>
