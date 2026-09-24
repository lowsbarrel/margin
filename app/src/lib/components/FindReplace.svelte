<script lang="ts">
	import { onDestroy, onMount } from 'svelte';
	import { CaseSensitive, ChevronDown, ChevronUp, Replace, ReplaceAll, X } from '@lucide/svelte';
	import * as m from '$lib/paraglide/messages.js';
	import type { EditorView } from '@codemirror/view';

	interface Props {
		view: EditorView;
		showReplace: boolean;
		ontogglereplace: () => void;
		onclose: () => void;
	}

	let { view, showReplace, ontogglereplace, onclose }: Props = $props();

	type FindApi = typeof import('$lib/editor/live/find');

	let api: FindApi | null = null;
	let searchInput = $state<HTMLInputElement | null>(null);
	let searchValue = $state('');
	let replaceValue = $state('');
	let caseSensitive = $state(false);
	let stats = $state({ total: 0, index: 0 });
	let stopWatch: (() => void) | null = null;

	function refresh() {
		if (api) stats = api.findState(view);
	}

	function apply() {
		api?.setFindQuery(view, { search: searchValue, replace: replaceValue, caseSensitive });
		refresh();
	}

	function step(backwards: boolean) {
		if (backwards) api?.findPreviousMatch(view);
		else api?.findNextMatch(view);
		refresh();
	}

	function replaceCurrent() {
		api?.replaceCurrentMatch(view);
		refresh();
	}

	function replaceEvery() {
		api?.replaceEveryMatch(view);
		refresh();
	}

	function close() {
		stopWatch?.();
		stopWatch = null;
		api?.closeFind(view);
		onclose();
	}

	function handleKeydown(event: KeyboardEvent) {
		if (event.key === 'Escape') {
			event.preventDefault();
			close();
		} else if (event.key === 'Enter') {
			event.preventDefault();
			step(event.shiftKey);
		}
	}

	onMount(() => {
		let alive = true;
		void (async () => {
			const mod = await import('$lib/editor/live/find');
			if (!alive) return;
			api = mod;
			mod.openFind(view);
			stopWatch = mod.watchFind(view, refresh);
			searchInput?.focus();
			searchInput?.select();
		})();
		return () => {
			alive = false;
		};
	});

	onDestroy(() => {
		stopWatch?.();
		stopWatch = null;
	});
</script>

<!-- svelte-ignore a11y_no_static_element_interactions -->
<div
	class="absolute top-2 right-4 z-60 flex min-w-[320px] flex-col gap-1 rounded-sm border border-border bg-surface-1 px-2 py-1.5 shadow-(--shadow-lg)"
	onkeydown={handleKeydown}
>
	<div class="flex items-center gap-1.5">
		<div class="flex flex-1 items-center overflow-hidden">
			<input
				bind:this={searchInput}
				bind:value={searchValue}
				oninput={apply}
				class="flex-1 border-none bg-transparent px-2 py-1 font-sans text-sm text-foreground caret-foreground outline-none placeholder:text-subtle-foreground"
				placeholder={m.find_placeholder()}
				spellcheck="false"
			/>
			<button
				class="mr-0.5 flex size-6 items-center justify-center rounded-xs border-none bg-transparent p-0 text-subtle-foreground transition-colors duration-120 ease-out hover:text-foreground {caseSensitive
					? 'bg-surface-2 text-accent-foreground'
					: ''}"
				onclick={() => {
					caseSensitive = !caseSensitive;
					apply();
				}}
				title={m.find_case_sensitive()}
			>
				<CaseSensitive size={14} />
			</button>
		</div>
		<span class="min-w-12.5 text-center text-xs whitespace-nowrap text-subtle-foreground">
			{#if searchValue && stats.total > 0}
				{stats.index} / {stats.total}
			{:else if searchValue}
				{m.find_no_results()}
			{/if}
		</span>
		<div class="flex gap-0.5">
			<button
				class="flex size-6.5 items-center justify-center rounded-xs border-none bg-transparent p-0 text-subtle-foreground transition-colors duration-120 ease-out enabled:hover:bg-surface-3 enabled:hover:text-foreground disabled:cursor-not-allowed disabled:opacity-30"
				onclick={() => step(true)}
				title={m.find_previous()}
				disabled={stats.total === 0}
			>
				<ChevronUp size={16} />
			</button>
			<button
				class="flex size-6.5 items-center justify-center rounded-xs border-none bg-transparent p-0 text-subtle-foreground transition-colors duration-120 ease-out enabled:hover:bg-surface-3 enabled:hover:text-foreground disabled:cursor-not-allowed disabled:opacity-30"
				onclick={() => step(false)}
				title={m.find_next()}
				disabled={stats.total === 0}
			>
				<ChevronDown size={16} />
			</button>
			<button
				class="flex size-6.5 items-center justify-center rounded-xs border-none bg-transparent p-0 text-subtle-foreground transition-colors duration-120 ease-out enabled:hover:bg-surface-3 enabled:hover:text-foreground disabled:cursor-not-allowed disabled:opacity-30 {showReplace
					? 'bg-surface-2 text-foreground'
					: ''}"
				onclick={ontogglereplace}
				title={m.find_toggle_replace()}
			>
				<Replace size={14} />
			</button>
			<button
				class="flex size-6.5 items-center justify-center rounded-xs border-none bg-transparent p-0 text-subtle-foreground transition-colors duration-120 ease-out enabled:hover:bg-surface-3 enabled:hover:text-foreground disabled:cursor-not-allowed disabled:opacity-30"
				onclick={close}
				title={m.find_close()}
			>
				<X size={16} />
			</button>
		</div>
	</div>

	{#if showReplace}
		<div class="flex items-center gap-1.5">
			<div class="flex flex-1 items-center overflow-hidden">
				<input
					bind:value={replaceValue}
					oninput={apply}
					class="flex-1 border-none bg-transparent px-2 py-1 font-sans text-sm text-foreground caret-foreground outline-none placeholder:text-subtle-foreground"
					placeholder={m.find_replace_placeholder()}
					spellcheck="false"
				/>
			</div>
			<div class="flex gap-0.5">
				<button
					class="flex size-6.5 items-center justify-center rounded-xs border-none bg-transparent p-0 text-subtle-foreground transition-colors duration-120 ease-out enabled:hover:bg-surface-3 enabled:hover:text-foreground disabled:cursor-not-allowed disabled:opacity-30"
					onclick={replaceCurrent}
					title={m.find_replace()}
					disabled={stats.total === 0}
				>
					<Replace size={14} />
				</button>
				<button
					class="flex size-6.5 items-center justify-center rounded-xs border-none bg-transparent p-0 text-subtle-foreground transition-colors duration-120 ease-out enabled:hover:bg-surface-3 enabled:hover:text-foreground disabled:cursor-not-allowed disabled:opacity-30"
					onclick={replaceEvery}
					title={m.find_replace_all()}
					disabled={stats.total === 0}
				>
					<ReplaceAll size={14} />
				</button>
			</div>
		</div>
	{/if}
</div>
