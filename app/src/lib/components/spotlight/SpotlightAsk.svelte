<script lang="ts">
	import { Sparkles, Wrench } from '@lucide/svelte';
	import AskAnswer from '$lib/components/AskAnswer.svelte';
	import { ask } from '$lib/stores/ask.svelte';
	import * as m from '$lib/paraglide/messages.js';

	interface Props {
		askedQuestion: string;
		onsettings: () => void;
		oncite: (title: string) => void;
	}

	let { askedQuestion, onsettings, oncite }: Props = $props();
</script>

<div class="flex min-h-0 flex-1 flex-col">
	{#if ask.configured === false}
		<div class="flex flex-col items-start gap-3 px-4 py-8">
			<p class="m-0 text-sm text-muted-foreground">{m.spotlight_ask_not_configured()}</p>
			<button
				type="button"
				onclick={onsettings}
				class="flex items-center gap-1.5 rounded-xs border border-border px-2.5 py-1.5 text-xs text-foreground transition-colors hover:bg-surface-3"
			>
				<Sparkles size={13} />
				{m.spotlight_ask_open_settings()}
			</button>
		</div>
	{:else if !askedQuestion}
		<div class="px-4 py-8 text-center text-sm text-subtle-foreground">
			{m.spotlight_ask_hint()}
		</div>
	{:else}
		{#if ask.steps.length > 0}
			<div class="flex flex-col gap-1 border-b border-border px-4 py-2.5">
				{#each ask.steps as step, i (i)}
					<div class="flex items-center gap-2 text-xs text-subtle-foreground">
						<Wrench size={12} class="shrink-0" />
						<span class="min-w-0 truncate">{step.summary}</span>
					</div>
				{/each}
			</div>
		{/if}

		<div class="min-h-0 flex-1 overflow-y-auto px-4 py-3">
			{#if ask.answer}
				<AskAnswer markdown={ask.answer} {oncite} />
			{:else if ask.running}
				<p class="m-0 text-sm text-subtle-foreground">{m.spotlight_ask_thinking()}</p>
			{:else if !ask.error}
				<p class="m-0 text-sm text-subtle-foreground">{m.spotlight_ask_empty()}</p>
			{/if}
			{#if ask.error}
				<p class="m-0 mt-2 font-sans text-xs text-destructive">
					{m.spotlight_ask_error({ error: ask.error })}
				</p>
			{/if}
		</div>

		{#if ask.running}
			<div class="flex items-center justify-between border-t border-border px-4 py-2">
				<span class="text-xs text-subtle-foreground">{askedQuestion}</span>
				<button
					type="button"
					onclick={() => ask.cancel()}
					class="shrink-0 rounded-xs border border-border px-2 py-1 text-xs text-muted-foreground transition-colors hover:bg-surface-3 hover:text-foreground"
				>
					{m.spotlight_ask_cancel()}
				</button>
			</div>
		{/if}
	{/if}
</div>
