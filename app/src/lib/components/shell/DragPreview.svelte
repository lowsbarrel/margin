<script lang="ts">
	import { drag } from '$lib/stores/drag.svelte';
</script>

{#if drag.active}
	<div
		class="pointer-events-none fixed z-9999 rounded-sm border border-border bg-background px-2.5 py-1.25 text-xs font-medium whitespace-nowrap text-foreground shadow-(--shadow-lg)"
		style="left: {drag.x + 14}px; top: {drag.y + 14}px"
	>
		{drag.item?.label}
	</div>
{/if}
