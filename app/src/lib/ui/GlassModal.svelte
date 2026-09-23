<script lang="ts">
	import { X } from '@lucide/svelte';
	import type { Snippet } from 'svelte';
	import * as Dialog from '$lib/components/ui/dialog/index.js';
	import * as m from '$lib/paraglide/messages.js';

	interface Props {
		title: string;
		onclose: () => void;
		children: Snippet;
		width?: string;
	}

	let { title, onclose, children, width = '560px' }: Props = $props();

	function handleOpenChange(next: boolean) {
		if (!next) onclose();
	}
</script>

<Dialog.Root open onOpenChange={handleOpenChange}>
	<Dialog.Content
		showCloseButton={false}
		style="width: {width}"
		class="flex max-h-[calc(100vh-80px)] max-w-[calc(100vw-32px)] animate-in flex-col gap-0 overflow-y-auto rounded-lg border border-border bg-background p-0 shadow-(--shadow-lg) ring-0 fade-in-0 slide-in-from-bottom-2 sm:max-w-[calc(100vw-32px)]"
	>
		<Dialog.Header
			class="sticky top-0 z-2 flex shrink-0 flex-row items-center justify-between gap-0 rounded-t-lg border-b border-border bg-background px-6 py-4"
		>
			<Dialog.Title class="text-base leading-5.5 font-semibold tracking-tight text-foreground">
				{title}
			</Dialog.Title>
			<Dialog.Close
				aria-label={m.dialog_close()}
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
