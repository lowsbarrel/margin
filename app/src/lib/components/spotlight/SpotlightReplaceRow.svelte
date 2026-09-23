<script lang="ts">
	import { Replace, ReplaceAll } from '@lucide/svelte';
	import * as m from '$lib/paraglide/messages.js';

	interface Props {
		replaceQuery: string;
		canReplace: boolean;
		replacing: boolean;
		count: number;
		onreplaceall: () => void;
	}

	let { replaceQuery = $bindable(), canReplace, replacing, count, onreplaceall }: Props = $props();
</script>

<div class="flex items-center gap-2.5 border-b border-border px-4 py-2.5">
	<Replace size={14} class="shrink-0 text-subtle-foreground" />
	<input
		bind:value={replaceQuery}
		placeholder={m.spotlight_replace_placeholder()}
		type="text"
		spellcheck="false"
		autocomplete="off"
		class="min-w-0 flex-1 border-none bg-transparent p-0 text-sm text-foreground caret-(--color-bg-brand) shadow-none outline-none placeholder:text-subtle-foreground"
	/>
	<button
		type="button"
		onclick={onreplaceall}
		disabled={!canReplace || replacing}
		title={m.spotlight_replace_all_files()}
		class="flex shrink-0 items-center gap-1.5 rounded-xs border border-border px-2 py-1 text-xs text-muted-foreground transition-colors hover:bg-surface-3 hover:text-foreground disabled:cursor-not-allowed disabled:opacity-40 disabled:hover:bg-transparent"
	>
		<ReplaceAll size={13} />
		{m.spotlight_replace_all_short({ count: String(count) })}
	</button>
</div>
