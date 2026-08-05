<script lang="ts">
	import { onDestroy } from 'svelte';
	import type { Editor } from '@tiptap/core';
	import { ChevronDown, ChevronUp, X, Replace, ReplaceAll, CaseSensitive } from '@lucide/svelte';
	import * as m from '$lib/paraglide/messages.js';

	interface Props {
		editor: Editor | null;
		showReplace?: boolean;
		onclose: () => void;
	}

	let { editor, showReplace = false, onclose }: Props = $props();

	let searchInput = $state<HTMLInputElement | null>(null);
	// Follows the `showReplace` prop, but stays user-overridable: the replace
	// toggle in the toolbar assigns to it, and that assignment holds until the
	// prop changes again (a writable $derived, not $state mirrored by an effect).
	let replaceVisible = $derived(showReplace);
	let searchValue = $state('');
	let replaceValue = $state('');
	let caseSensitive = $state(false);
	let totalMatches = $state(0);
	let currentIndex = $state(0);

	function syncMatchState() {
		const storage = editor?.storage.searchReplace;
		if (storage) {
			totalMatches = storage.totalMatches ?? 0;
			currentIndex = storage.currentIndex ?? 0;
		} else {
			totalMatches = 0;
			currentIndex = 0;
		}
	}

	$effect(() => {
		if (searchInput) {
			searchInput.focus();
			searchInput.select();
		}
	});

	// The match search runs asynchronously (Rust IPC), so `storage.totalMatches`
	// is only populated a tick or two after a command dispatches — reading it
	// synchronously right after `setSearchTerm()` always yields the *previous*
	// query's numbers. Mirror it off every transaction instead, otherwise the
	// counter lags a keystroke behind and the next/prev buttons (disabled on
	// `totalMatches === 0`) stay greyed out on a query that does have matches.
	$effect(() => {
		const ed = editor;
		if (!ed) {
			totalMatches = 0;
			currentIndex = 0;
			return;
		}
		syncMatchState();
		ed.on('transaction', syncMatchState);
		return () => {
			ed.off('transaction', syncMatchState);
		};
	});

	let searchDebounceTimer: ReturnType<typeof setTimeout>;

	function handleSearchInput() {
		clearTimeout(searchDebounceTimer);
		searchDebounceTimer = setTimeout(() => {
			if (!editor) return;
			editor.commands.setSearchTerm(searchValue);
			syncMatchState();
		}, 100);
	}

	function handleReplaceInput() {
		if (!editor) return;
		editor.commands.setReplaceTerm(replaceValue);
	}

	function toggleCaseSensitive() {
		caseSensitive = !caseSensitive;
		if (!editor) return;
		editor.commands.setCaseSensitive(caseSensitive);
		syncMatchState();
	}

	function findNext() {
		if (!editor) return;
		editor.commands.findNext();
		syncMatchState();
	}

	function findPrev() {
		if (!editor) return;
		editor.commands.findPrev();
		syncMatchState();
	}

	function replaceCurrent() {
		if (!editor) return;
		editor.commands.setReplaceTerm(replaceValue);
		editor.commands.replaceCurrent();
		syncMatchState();
	}

	function replaceAll() {
		if (!editor) return;
		editor.commands.setReplaceTerm(replaceValue);
		editor.commands.replaceAll();
		syncMatchState();
	}

	function close() {
		clearTimeout(searchDebounceTimer);
		if (editor) {
			editor.commands.clearSearch();
		}
		onclose();
	}

	onDestroy(() => {
		clearTimeout(searchDebounceTimer);
	});

	function handleKeydown(e: KeyboardEvent) {
		if (e.key === 'Escape') {
			e.preventDefault();
			close();
		} else if (e.key === 'Enter' && !e.shiftKey) {
			e.preventDefault();
			findNext();
		} else if (e.key === 'Enter' && e.shiftKey) {
			e.preventDefault();
			findPrev();
		}
	}
</script>

<!-- svelte-ignore a11y_no_static_element_interactions -->
<!-- Floats over the editor, so it earns a shadow — on a solid surface with a
     hairline border, per the house style. -->
<div
	class="absolute top-2 right-4 z-60 flex min-w-[320px] flex-col gap-1 rounded-sm border border-border bg-surface-1 px-2 py-1.5 shadow-[var(--shadow-lg)]"
	onkeydown={handleKeydown}
>
	<div class="flex items-center gap-1.5">
		<div class="flex flex-1 items-center overflow-hidden">
			<input
				bind:this={searchInput}
				bind:value={searchValue}
				oninput={handleSearchInput}
				class="flex-1 border-none bg-transparent px-2 py-1 font-sans text-sm text-foreground caret-foreground outline-none placeholder:text-subtle-foreground"
				placeholder={m.find_placeholder()}
				spellcheck="false"
			/>
			<button
				class="mr-0.5 flex size-6 items-center justify-center rounded-xs border-none bg-transparent p-0 text-subtle-foreground transition-colors duration-120 ease-out hover:text-foreground {caseSensitive
					? 'bg-surface-2 text-accent-foreground'
					: ''}"
				onclick={toggleCaseSensitive}
				title={m.find_case_sensitive()}
			>
				<CaseSensitive size={14} />
			</button>
		</div>
		<span class="min-w-[50px] text-center text-xs whitespace-nowrap text-subtle-foreground">
			{#if searchValue && totalMatches > 0}
				{currentIndex + 1} / {totalMatches}
			{:else if searchValue}
				{m.find_no_results()}
			{/if}
		</span>
		<div class="flex gap-0.5">
			<button
				class="flex size-[26px] items-center justify-center rounded-xs border-none bg-transparent p-0 text-subtle-foreground transition-colors duration-120 ease-out enabled:hover:bg-surface-3 enabled:hover:text-foreground disabled:cursor-not-allowed disabled:opacity-30"
				onclick={findPrev}
				title={m.find_previous()}
				disabled={totalMatches === 0}
			>
				<ChevronUp size={16} />
			</button>
			<button
				class="flex size-[26px] items-center justify-center rounded-xs border-none bg-transparent p-0 text-subtle-foreground transition-colors duration-120 ease-out enabled:hover:bg-surface-3 enabled:hover:text-foreground disabled:cursor-not-allowed disabled:opacity-30"
				onclick={findNext}
				title={m.find_next()}
				disabled={totalMatches === 0}
			>
				<ChevronDown size={16} />
			</button>
			<button
				class="flex size-[26px] items-center justify-center rounded-xs border-none bg-transparent p-0 text-subtle-foreground transition-colors duration-120 ease-out enabled:hover:bg-surface-3 enabled:hover:text-foreground disabled:cursor-not-allowed disabled:opacity-30 {replaceVisible
					? 'bg-surface-2 text-foreground'
					: ''}"
				onclick={() => (replaceVisible = !replaceVisible)}
				title={m.find_toggle_replace()}
			>
				<Replace size={14} />
			</button>
			<button
				class="flex size-[26px] items-center justify-center rounded-xs border-none bg-transparent p-0 text-subtle-foreground transition-colors duration-120 ease-out enabled:hover:bg-surface-3 enabled:hover:text-foreground disabled:cursor-not-allowed disabled:opacity-30"
				onclick={close}
				title={m.find_close()}
			>
				<X size={16} />
			</button>
		</div>
	</div>

	{#if replaceVisible}
		<div class="flex items-center gap-1.5">
			<div class="flex flex-1 items-center overflow-hidden">
				<input
					bind:value={replaceValue}
					oninput={handleReplaceInput}
					class="flex-1 border-none bg-transparent px-2 py-1 font-sans text-sm text-foreground caret-foreground outline-none placeholder:text-subtle-foreground"
					placeholder={m.find_replace_placeholder()}
					spellcheck="false"
				/>
			</div>
			<div class="flex gap-0.5">
				<button
					class="flex size-[26px] items-center justify-center rounded-xs border-none bg-transparent p-0 text-subtle-foreground transition-colors duration-120 ease-out enabled:hover:bg-surface-3 enabled:hover:text-foreground disabled:cursor-not-allowed disabled:opacity-30"
					onclick={replaceCurrent}
					title={m.find_replace()}
					disabled={totalMatches === 0}
				>
					<Replace size={14} />
				</button>
				<button
					class="flex size-[26px] items-center justify-center rounded-xs border-none bg-transparent p-0 text-subtle-foreground transition-colors duration-120 ease-out enabled:hover:bg-surface-3 enabled:hover:text-foreground disabled:cursor-not-allowed disabled:opacity-30"
					onclick={replaceAll}
					title={m.find_replace_all()}
					disabled={totalMatches === 0}
				>
					<ReplaceAll size={14} />
				</button>
			</div>
		</div>
	{/if}
</div>
