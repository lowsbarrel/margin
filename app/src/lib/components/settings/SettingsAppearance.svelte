<script lang="ts">
	import { Section } from '$lib/ui';
	import { theme } from '$lib/stores/theme.svelte';
	import { Palette, Sun, Moon } from '@lucide/svelte';
	import { cn } from '$lib/utils';
	import * as m from '$lib/paraglide/messages.js';

	const SEGMENT =
		'flex cursor-pointer items-center justify-center gap-2 rounded-[9px] border-none bg-transparent px-3 py-1.75 font-sans text-sm font-medium tracking-normal transition-colors duration-120 ease-out hover:text-foreground';

	const SEGMENT_SELECTED = 'bg-background text-foreground shadow-(--shadow-sm)';
</script>

<Section title={m.appearance_title()} icon={Palette} collapsible defaultOpen={false}>
	<div
		class="grid grid-cols-2 gap-0.5 rounded-md border border-border bg-surface-1 p-0.5"
		role="radiogroup"
		aria-label={m.appearance_title()}
	>
		<button
			class={cn(SEGMENT, theme.current === 'dark' ? SEGMENT_SELECTED : 'text-muted-foreground')}
			role="radio"
			aria-checked={theme.current === 'dark'}
			onclick={() => theme.set('dark')}
		>
			<Moon size={14} />
			{m.appearance_dark()}
		</button>
		<button
			class={cn(SEGMENT, theme.current === 'light' ? SEGMENT_SELECTED : 'text-muted-foreground')}
			role="radio"
			aria-checked={theme.current === 'light'}
			onclick={() => theme.set('light')}
		>
			<Sun size={14} />
			{m.appearance_light()}
		</button>
	</div>
	<p class="m-0 font-sans text-xs text-subtle-foreground">{m.appearance_hint()}</p>
</Section>
