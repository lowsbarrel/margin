<script lang="ts">
	import { ChevronDown } from '@lucide/svelte';
	import type { Snippet } from 'svelte';

	interface Props {
		value?: string;
		onchange?: (value: string) => void;
		id?: string;
		disabled?: boolean;
		children: Snippet;
	}

	let { value = $bindable(''), onchange, id, disabled = false, children }: Props = $props();

	function handleChange(event: Event) {
		value = (event.target as HTMLSelectElement).value;
		onchange?.(value);
	}
</script>

<div class="relative flex w-full items-center">
	<select
		{id}
		{disabled}
		{value}
		onchange={handleChange}
		class="h-8.5 w-full cursor-pointer appearance-none rounded-sm border border-input bg-background py-0 pr-8.5 pl-3 font-sans text-sm text-foreground transition-colors duration-120 ease-out outline-none focus-visible:border-ring focus-visible:ring-3 focus-visible:ring-ring/50 disabled:cursor-not-allowed disabled:opacity-50"
	>
		{@render children()}
	</select>
	<span class="pointer-events-none absolute right-2.5 flex items-center text-subtle-foreground">
		<ChevronDown size={14} />
	</span>
</div>
