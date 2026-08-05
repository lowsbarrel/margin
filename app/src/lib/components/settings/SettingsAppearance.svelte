<script lang="ts">
	import { Section } from '$lib/ui';
	import { theme } from '$lib/stores/theme.svelte';
	import { Palette, Sun, Moon } from '@lucide/svelte';
	import { cn } from '$lib/utils';
	import * as m from '$lib/paraglide/messages.js';

	/* `rounded-[9px]` is the track's 12px radius less its 2px padding and 1px
	   border, so the segment's corner sits concentric inside the track. The
	   hover colour is applied unconditionally: it is the same colour the
	   selected segment already carries, so it is a no-op there — which is what
	   the old `:hover:not(.selected)` rule expressed. */
	const SEGMENT =
		'flex cursor-pointer items-center justify-center gap-2 rounded-[9px] border-none bg-transparent px-3 py-[7px] font-sans text-sm font-medium tracking-normal transition-colors duration-120 ease-out hover:text-foreground';

	/* The one place a shadow is warranted: the active segment genuinely floats
	   above the track. */
	const SEGMENT_SELECTED = 'bg-background text-foreground shadow-[var(--shadow-sm)]';
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
