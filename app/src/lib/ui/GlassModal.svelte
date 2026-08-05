<script lang="ts">
	import { X } from '@lucide/svelte';
	import type { Snippet } from 'svelte';
	import * as Dialog from '$lib/components/ui/dialog/index.js';

	interface Props {
		title: string;
		onclose: () => void;
		children: Snippet;
		width?: string;
	}

	let { title, onclose, children, width = '560px' }: Props = $props();

	/* The caller owns the mounted/unmounted state — this component is rendered
	   inside an `{#if}`. So the dialog is opened on mount and every close route
	   bits-ui offers (Escape, backdrop click, the X button) funnels through
	   `onOpenChange` back to `onclose`. */
	function handleOpenChange(next: boolean) {
		if (!next) onclose();
	}
</script>

<Dialog.Root open onOpenChange={handleOpenChange}>
	<!-- The panel genuinely floats, so this is one of the few places a shadow is
	     warranted — paired with a hairline border to keep the edge crisp. The
	     inline width beats shadcn's `w-full`; the max-widths replace its
	     `sm:max-w-sm` cap, which would otherwise shrink the modal to 384px. -->
	<Dialog.Content
		showCloseButton={false}
		style="width: {width}"
		class="flex max-h-[calc(100vh-80px)] max-w-[calc(100vw-32px)] animate-in flex-col gap-0 overflow-y-auto rounded-lg border border-border bg-background p-0 shadow-[var(--shadow-lg)] ring-0 fade-in-0 slide-in-from-bottom-2 sm:max-w-[calc(100vw-32px)]"
	>
		<Dialog.Header
			class="sticky top-0 z-2 flex shrink-0 flex-row items-center justify-between gap-0 rounded-t-lg border-b border-border bg-background px-6 py-4"
		>
			<Dialog.Title class="text-base leading-[22px] font-semibold tracking-tight text-foreground">
				{title}
			</Dialog.Title>
			<Dialog.Close
				aria-label="Close"
				class="flex size-7 shrink-0 cursor-pointer items-center justify-center rounded-sm bg-transparent p-0 text-subtle-foreground transition-colors duration-120 ease-out hover:bg-muted hover:text-foreground"
			>
				<X size={16} />
			</Dialog.Close>
		</Dialog.Header>
		<div class="flex shrink-0 flex-col gap-3 p-6">
			{@render children()}
		</div>
	</Dialog.Content>
</Dialog.Root>

<style>
	/* The only rule here that no utility can express: `tw-animate-css` is not
	   installed, so shadcn's `animate-in`/`zoom-in-95` classes are inert and
	   there is no built-in enter keyframe. Declared `-global-` because the
	   animated element belongs to <Dialog.Content>, which Svelte's scoping
	   classes do not reach.

	   Safe to animate `transform` here: Tailwind v4 centres the panel with the
	   separate `translate` property, so the two compose instead of colliding. */
	@keyframes -global-margin-modal-in {
		from {
			opacity: 0;
			transform: translateY(8px);
		}
		to {
			opacity: 1;
			transform: translateY(0);
		}
	}
</style>
