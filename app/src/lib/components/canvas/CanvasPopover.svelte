<script lang="ts">
	import type { Snippet } from 'svelte';

	interface Props {
		onclose: () => void;
		children: Snippet;
	}

	let { onclose, children }: Props = $props();
</script>

<!-- svelte-ignore a11y_no_static_element_interactions -->
<div class="fixed inset-0 z-20" onmousedown={onclose}></div>
<div
	class="absolute bottom-full left-1/2 z-30 mb-2 -translate-x-1/2 rounded-sm border border-border bg-background p-1.5 shadow-(--shadow-lg)"
>
	{@render children()}
</div>
