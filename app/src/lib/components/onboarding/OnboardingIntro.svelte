<script lang="ts">
	import type { Component } from 'svelte';
	import { fly, type FlyParams } from 'svelte/transition';
	import { cubicIn, cubicOut } from 'svelte/easing';
	import { ArrowLeft, ArrowRight } from '@lucide/svelte';
	import { Button } from '$lib/ui';
	import { motion } from '$lib/stores/motion.svelte';
	import LogoMark from './LogoMark.svelte';
	import IntroArtFiles from './IntroArtFiles.svelte';
	import IntroArtPassphrase from './IntroArtPassphrase.svelte';
	import IntroArtSync from './IntroArtSync.svelte';
	import IntroArtFind from './IntroArtFind.svelte';
	import IntroArtHistory from './IntroArtHistory.svelte';
	import * as m from '$lib/paraglide/messages.js';

	interface Props {
		onDone: () => void;
	}

	let { onDone }: Props = $props();

	const steps = $derived([
		{
			id: 'files',
			art: IntroArtFiles as Component,
			title: m.onboarding_files_title(),
			body: m.onboarding_files_body()
		},
		{
			id: 'passphrase',
			art: IntroArtPassphrase as Component,
			title: m.onboarding_passphrase_title(),
			body: m.onboarding_passphrase_body()
		},
		{
			id: 'sync',
			art: IntroArtSync as Component,
			title: m.onboarding_sync_title(),
			body: m.onboarding_sync_body()
		},
		{
			id: 'find',
			art: IntroArtFind as Component,
			title: m.onboarding_find_title(),
			body: m.onboarding_find_body()
		},
		{
			id: 'history',
			art: IntroArtHistory as Component,
			title: m.onboarding_history_title(),
			body: m.onboarding_history_body()
		}
	]);

	let step = $state(0);
	let isLast = $derived(step === steps.length - 1);
	let Art = $derived(steps[step].art);

	// Reduced motion keeps the swap legible but drops the travel: `fly` with no
	// offset is a plain fade.
	let enter: FlyParams = $derived(
		motion.reduced ? { duration: 150 } : { y: 16, duration: 260, easing: cubicOut }
	);
	let leave: FlyParams = $derived(
		motion.reduced ? { duration: 150 } : { y: -16, duration: 200, easing: cubicIn }
	);

	function go(to: number) {
		if (to >= 0 && to < steps.length) step = to;
	}

	function next() {
		if (isLast) onDone();
		else go(step + 1);
	}

	function back() {
		go(step - 1);
	}

	function handleKey(e: KeyboardEvent) {
		// A focused button already turns Enter into a click; acting on it here too
		// would advance two steps.
		const onControl =
			e.target instanceof HTMLElement &&
			(e.target.tagName === 'BUTTON' || e.target.tagName === 'A');
		if (e.key === 'Enter' && onControl) return;

		if (e.key === 'Enter' || e.key === 'ArrowRight') {
			e.preventDefault();
			next();
		} else if (e.key === 'ArrowLeft') {
			e.preventDefault();
			back();
		} else if (e.key === 'Escape') {
			e.preventDefault();
			onDone();
		}
	}
</script>

<svelte:window onkeydown={handleKey} />

<div class="flex w-full flex-col items-center">
	<div class="mb-6 flex flex-col items-center gap-1 text-accent-foreground">
		<LogoMark size={54} />
		<h1 class="font-sans text-xl font-bold tracking-[0.08em] text-foreground">{m.app_name()}</h1>
	</div>

	<!-- One grid cell holds both the outgoing and incoming step, so the card
	     keeps its height while the transition plays. -->
	<div class="grid w-full">
		{#key step}
			<div in:fly={enter} out:fly={leave} class="flex flex-col items-center gap-4 [grid-area:1/1]">
				<div class="flex h-[150px] w-full items-center justify-center">
					<div class="w-full max-w-[220px]">
						<Art />
					</div>
				</div>
				<h2 class="min-h-[26px] text-center font-sans text-lg font-semibold text-foreground">
					{steps[step].title}
				</h2>
				<p class="min-h-[60px] text-center font-sans text-sm leading-normal text-muted-foreground">
					{steps[step].body}
				</p>
			</div>
		{/key}
	</div>

	<div class="mt-6 flex h-1.5 items-center gap-1.5">
		{#each steps as s, i (s.id)}
			<button
				class="h-1.5 rounded-full p-0! transition-[width,background-color] duration-120 {i === step
					? 'w-4 bg-brand'
					: 'w-1.5 bg-hairline hover:bg-subtle-foreground'}"
				onclick={() => go(i)}
				aria-label={m.onboarding_step_of({ current: String(i + 1), total: String(steps.length) })}
				aria-current={i === step ? 'step' : undefined}
			></button>
		{/each}
	</div>

	<div class="mt-5 flex w-full items-center justify-between gap-3">
		<Button variant="ghost" size="sm" onclick={onDone}>{m.onboarding_skip()}</Button>
		<div class="flex items-center gap-1.5">
			{#if step > 0}
				<Button variant="ghost" size="sm" icon={ArrowLeft} onclick={back}
					>{m.onboarding_back()}</Button
				>
			{/if}
			<Button variant="primary" size="sm" icon={isLast ? undefined : ArrowRight} onclick={next}>
				{isLast ? m.onboarding_get_started() : m.onboarding_next()}
			</Button>
		</div>
	</div>
</div>
