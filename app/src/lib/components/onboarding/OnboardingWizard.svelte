<script lang="ts">
	import type { Snippet } from 'svelte';
	import { fly, type FlyParams } from 'svelte/transition';
	import { cubicIn, cubicOut } from 'svelte/easing';
	import { ArrowLeft, ArrowRight } from '@lucide/svelte';
	import { Button } from '$lib/ui';
	import { motion } from '$lib/stores/motion.svelte';
	import * as m from '$lib/paraglide/messages.js';

	interface Props {
		stepIds: string[];
		step: number;
		canAdvance: boolean;
		exitLabel?: string;
		onExit?: () => void;
		children: Snippet<[string]>;
		finalAction: Snippet;
	}

	let {
		stepIds,
		step = $bindable(),
		canAdvance,
		exitLabel,
		onExit,
		children,
		finalAction
	}: Props = $props();

	let isLast = $derived(step === stepIds.length - 1);

	let enter: FlyParams = $derived(
		motion.reduced ? { duration: 150 } : { x: 18, duration: 240, easing: cubicOut }
	);
	let leave: FlyParams = $derived(
		motion.reduced ? { duration: 150 } : { x: -18, duration: 180, easing: cubicIn }
	);

	function go(to: number) {
		if (to >= 0 && to < stepIds.length) step = to;
	}

	function advance() {
		if (canAdvance) go(step + 1);
	}

	function handleKey(e: KeyboardEvent) {
		// Carets live in these steps: arrows and Enter belong to the field while
		// it has focus.
		const el = e.target;
		if (
			el instanceof HTMLInputElement ||
			el instanceof HTMLTextAreaElement ||
			(el instanceof HTMLElement && el.isContentEditable)
		)
			return;

		if (e.key === 'Escape') {
			if (!onExit) return;
			e.preventDefault();
			onExit();
			return;
		}

		const onControl = el instanceof HTMLElement && el.tagName === 'BUTTON';
		if (e.key === 'Enter' && onControl) return;

		if (e.key === 'Enter' || e.key === 'ArrowRight') {
			if (isLast) return;
			e.preventDefault();
			advance();
		} else if (e.key === 'ArrowLeft') {
			e.preventDefault();
			go(step - 1);
		}
	}
</script>

<svelte:window onkeydown={handleKey} />

<div class="flex w-full flex-col">
	{#if exitLabel && onExit}
		<button
			class="mb-1 flex items-center gap-1.5 self-start bg-transparent p-0 text-sm text-muted-foreground transition-colors hover:text-foreground"
			onclick={onExit}
		>
			<ArrowLeft size={14} />
			{exitLabel}
		</button>
	{/if}

	<div class="grid w-full">
		{#key step}
			<div in:fly={enter} out:fly={leave} class="[grid-area:1/1]">
				{@render children(stepIds[step])}
			</div>
		{/key}
	</div>

	<div class="mt-5 flex h-1.5 items-center justify-center gap-1.5">
		{#each stepIds as id, i (id)}
			<button
				class="h-1.5 rounded-full p-0! transition-[width,background-color] duration-120 {i === step
					? 'w-4 bg-brand'
					: 'w-1.5 bg-hairline hover:bg-subtle-foreground'}"
				onclick={() => go(i)}
				aria-label={m.onboarding_step_of({ current: String(i + 1), total: String(stepIds.length) })}
				aria-current={i === step ? 'step' : undefined}
			></button>
		{/each}
	</div>

	<div class="mt-4 flex items-center justify-between gap-2">
		{#if step > 0}
			<Button variant="ghost" size="sm" icon={ArrowLeft} onclick={() => go(step - 1)}>
				{m.onboarding_back()}
			</Button>
		{:else}
			<span></span>
		{/if}

		{#if isLast}
			{@render finalAction()}
		{:else}
			<Button
				variant="primary"
				size="sm"
				icon={ArrowRight}
				disabled={!canAdvance}
				onclick={advance}
			>
				{m.onboarding_next()}
			</Button>
		{/if}
	</div>
</div>
