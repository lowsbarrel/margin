<script lang="ts">
	import type { Snippet } from 'svelte';

	interface Props {
		label?: string;
		hint?: string;
		forId?: string;
		children: Snippet;
	}

	let { label, hint, forId, children }: Props = $props();
</script>

<div class="flex flex-col gap-1.5">
	{#if label}
		<label class="text-xs font-medium tracking-normal text-muted-foreground" for={forId}>
			{label}
		</label>
	{/if}
	{@render children()}
	{#if hint}
		<p class="m-0 font-sans text-xs text-subtle-foreground">{hint}</p>
	{/if}
</div>
