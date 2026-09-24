<script lang="ts">
	import TerminalView from '$lib/components/TerminalView.svelte';
	import { terminals } from '$lib/stores/terminals.svelte';
	import { IconButton } from '$lib/ui';
	import { Plus, X, ChevronDown } from '@lucide/svelte';
	import * as m from '$lib/paraglide/messages.js';

	// Never unmounted while closed: each mounted TerminalView owns a live PTY and its scrollback.
	let resizing = $state(false);

	function startResize(e: MouseEvent) {
		e.preventDefault();
		resizing = true;
		const startY = e.clientY;
		const startHeight = terminals.height;

		function onMove(ev: MouseEvent) {
			terminals.height = startHeight + (startY - ev.clientY);
		}
		function onUp() {
			resizing = false;
			window.removeEventListener('mousemove', onMove);
			window.removeEventListener('mouseup', onUp);
		}
		window.addEventListener('mousemove', onMove);
		window.addEventListener('mouseup', onUp);
	}
</script>

<div
	class="relative flex shrink-0 flex-col border-t border-border bg-surface-1"
	class:hidden={!terminals.open}
	style="height: {terminals.height}px"
>
	<!-- svelte-ignore a11y_no_static_element_interactions -->
	<div
		class="absolute -top-[3px] z-10 h-[6px] w-full cursor-row-resize transition-colors duration-120 ease-out {resizing
			? 'bg-brand/50'
			: 'hover:bg-brand/25'}"
		onmousedown={startResize}
	></div>

	<div class="flex h-9 min-h-9 items-center gap-1 border-b border-border px-2 select-none">
		<div class="flex min-w-0 flex-1 items-center gap-0.5 overflow-x-auto">
			{#each terminals.tabs as tab (tab.id)}
				<div
					class="flex h-7 min-w-0 cursor-pointer items-center gap-1.5 rounded-sm px-2 text-xs transition-colors duration-120 ease-out {tab.id ===
					terminals.activeId
						? 'bg-surface-3 text-foreground'
						: 'text-subtle-foreground hover:bg-surface-2 hover:text-muted-foreground'}"
					role="tab"
					tabindex={0}
					aria-selected={tab.id === terminals.activeId}
					onclick={() => terminals.activate(tab.id)}
					onkeydown={(e) => {
						if (e.key === 'Enter' || e.key === ' ') terminals.activate(tab.id);
					}}
				>
					<span class="max-w-32 min-w-0 truncate">{m.terminal_tab_title({ n: tab.n })}</span>
					{#if tab.exited !== null}
						<span class="shrink-0 text-subtle-foreground tabular-nums">{tab.exited}</span>
					{/if}
					<button
						class="flex size-4.5 shrink-0 cursor-pointer items-center justify-center rounded-xs bg-transparent p-0 text-subtle-foreground transition-colors hover:bg-surface-3 hover:text-foreground"
						onclick={(e) => {
							e.stopPropagation();
							terminals.close(tab.id);
						}}
						tabindex={-1}
						aria-label={m.terminal_close_tab()}
					>
						<X size={12} />
					</button>
				</div>
			{/each}
		</div>

		<IconButton icon={Plus} size="sm" onclick={() => terminals.create()} title={m.terminal_new()} />
		<IconButton
			icon={ChevronDown}
			size="sm"
			onclick={() => terminals.toggle()}
			title={m.terminal_hide()}
		/>
	</div>

	<div class="relative flex-1 overflow-hidden">
		{#each terminals.tabs as tab (tab.id)}
			{@const isActive = tab.id === terminals.activeId}
			<div class="absolute inset-0" class:hidden={!isActive}>
				<TerminalView {tab} visible={isActive && terminals.open} />
			</div>
		{/each}
	</div>
</div>
