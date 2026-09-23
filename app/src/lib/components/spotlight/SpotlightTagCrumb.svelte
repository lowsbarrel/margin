<script lang="ts">
	import { ChevronLeft } from '@lucide/svelte';
	import * as m from '$lib/paraglide/messages.js';

	interface Props {
		tag: string;
		count: number;
		onback: () => void;
	}

	let { tag, count, onback }: Props = $props();
</script>

<div class="flex items-center justify-between border-b border-border px-4 py-2">
	<button
		type="button"
		onclick={onback}
		class="flex items-center gap-1.5 text-xs text-muted-foreground transition-colors hover:text-foreground"
	>
		<ChevronLeft size={13} />
		<span class="font-mono text-accent-foreground">#{tag}</span>
	</button>
	<span class="text-xs text-subtle-foreground">
		{count}
		{m.tags_files({ count })}
	</span>
</div>
