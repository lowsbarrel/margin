<script module lang="ts">
	export interface LightboxImage {
		src: string;
		alt?: string;
	}
</script>

<script lang="ts">
	import { onMount } from 'svelte';
	import { ChevronLeft, ChevronRight, X } from '@lucide/svelte';
	import ImageViewer from '$lib/components/ImageViewer.svelte';
	import { fileNameFromSrc } from '$lib/utils/mime';
	import * as m from '$lib/paraglide/messages.js';

	interface Props {
		/** Every image of the note the lightbox was opened from, in document order. */
		images: LightboxImage[];
		index: number;
		onclose: () => void;
		onnavigate: (index: number) => void;
	}

	let { images, index, onclose, onnavigate }: Props = $props();

	let scrimEl = $state<HTMLDivElement>(undefined!);
	let current = $derived(images[Math.min(Math.max(index, 0), images.length - 1)]);
	let currentName = $derived(current ? current.alt || fileNameFromSrc(current.src) : '');

	function step(delta: number) {
		if (images.length < 2) return;
		onnavigate((index + delta + images.length) % images.length);
	}

	// The scrim takes focus so Escape and the arrows work before any click, which
	// a non-focusable div could never do.
	onMount(() => scrimEl?.focus());

	function handleKeydown(e: KeyboardEvent) {
		if (e.key === 'Escape') {
			e.preventDefault();
			onclose();
		} else if (e.key === 'ArrowLeft') {
			e.preventDefault();
			step(-1);
		} else if (e.key === 'ArrowRight') {
			e.preventDefault();
			step(1);
		}
	}
</script>

<!--
	The scrim and its chrome are deliberately theme-independent: a photo lightbox
	is always a dark room, in light mode as much as dark. So these use literal
	black/white with an alpha modifier rather than the semantic surface tokens,
	which would flip with `data-theme` and wash the image out.
-->
<div
	class="fixed inset-0 z-300 flex flex-col bg-black/85 backdrop-blur-[6px]"
	bind:this={scrimEl}
	role="dialog"
	aria-modal="true"
	aria-label={currentName}
	tabindex={-1}
	onkeydown={handleKeydown}
>
	{#if current}
		{#key current.src}
			<ImageViewer
				src={current.src}
				name={currentName}
				variant="lightbox"
				onbackgroundclick={onclose}
			/>
		{/key}
	{/if}

	{#if images.length > 1}
		<button
			class="absolute top-1/2 left-4 flex size-11 -translate-y-1/2 items-center justify-center rounded-full bg-transparent text-white/70 [transition:color_var(--transition-fast),background_var(--transition-fast)] hover:bg-white/10 hover:text-white"
			onclick={() => step(-1)}
			aria-label={m.viewer_previous()}
		>
			<ChevronLeft size={26} />
		</button>
		<button
			class="absolute top-1/2 right-4 flex size-11 -translate-y-1/2 items-center justify-center rounded-full bg-transparent text-white/70 [transition:color_var(--transition-fast),background_var(--transition-fast)] hover:bg-white/10 hover:text-white"
			onclick={() => step(1)}
			aria-label={m.viewer_next()}
		>
			<ChevronRight size={26} />
		</button>
		<span class="absolute top-5 left-6 text-sm text-white/60 tabular-nums">
			{index + 1} / {images.length}
		</span>
	{/if}

	<button
		class="absolute top-4 right-5 flex size-9 items-center justify-center rounded-sm bg-transparent text-white/70 [transition:color_var(--transition-fast),background_var(--transition-fast)] hover:bg-white/10 hover:text-white"
		onclick={onclose}
		aria-label={m.tab_close()}
	>
		<X size={20} />
	</button>
</div>
