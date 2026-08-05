<script lang="ts">
	interface Props {
		src: string;
		alt?: string;
		onclose: () => void;
	}

	let { src, alt = 'Image', onclose }: Props = $props();

	function handleKeydown(e: KeyboardEvent) {
		if (e.key === 'Escape') onclose();
	}
</script>

<!-- svelte-ignore a11y_no_static_element_interactions -->
<!--
	The scrim and its close button are deliberately theme-independent: a photo
	lightbox is always a dark room, in light mode as much as dark. So these use
	literal black/white with an alpha modifier rather than the semantic surface
	tokens, which would flip with `data-theme` and wash the image out.
-->
<div
	class="fixed inset-0 z-[300] flex cursor-zoom-out items-center justify-center bg-black/85 backdrop-blur-[6px]"
	onclick={onclose}
	onkeydown={handleKeydown}
>
	<!-- svelte-ignore a11y_no_noninteractive_element_interactions -->
	<!-- svelte-ignore a11y_click_events_have_key_events -->
	<img
		{src}
		{alt}
		class="max-h-[90vh] max-w-[90vw] cursor-default rounded-sm object-contain shadow-[var(--shadow-lg)] select-none"
		onclick={(e) => e.stopPropagation()}
	/>
	<button
		class="absolute top-4 right-5 rounded-sm bg-transparent px-2.5 py-1 text-3xl/none text-white/70 [transition:color_var(--transition-fast),background_var(--transition-fast)] hover:bg-white/10 hover:text-white"
		onclick={onclose}
		aria-label="Close">&times;</button
	>
</div>
