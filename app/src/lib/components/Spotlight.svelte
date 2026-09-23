<script lang="ts">
	import { onMount, onDestroy, tick, untrack } from 'svelte';
	import { ask } from '$lib/stores/ask.svelte';
	import { vault } from '$lib/stores/vault.svelte';
	import { tags as tagsStore } from '$lib/stores/tags.svelte';
	import { toast } from '$lib/stores/toast.svelte';
	import { searchFiles } from '$lib/fs/bridge';
	import { Loader, Replace, Search, Sparkles } from '@lucide/svelte';
	import * as m from '$lib/paraglide/messages.js';
	import SpotlightAsk from './spotlight/SpotlightAsk.svelte';
	import SpotlightReplaceRow from './spotlight/SpotlightReplaceRow.svelte';
	import SpotlightRow from './spotlight/SpotlightRow.svelte';
	import SpotlightTagCrumb from './spotlight/SpotlightTagCrumb.svelte';
	import { buildItems, itemKey, type SpotlightItem } from './spotlight/spotlight-items';
	import { SpotlightSearch } from './spotlight/spotlight-search.svelte';
	import { replaceInEveryFile, replaceInOneFile } from './spotlight/spotlight-replace';

	interface Props {
		onselect: (path: string, searchText?: string) => void;
		onclose: () => void;
		onsettings: () => void;
	}

	let { onselect, onclose, onsettings }: Props = $props();

	let query = $state('');
	let selectedIndex = $state(0);
	let inputEl = $state<HTMLInputElement | null>(null);
	let listEl = $state<HTMLDivElement | null>(null);
	let selectedTag = $state<string | null>(null);
	let showReplace = $state(false);
	let replaceQuery = $state('');
	let replacing = $state(false);
	let askedQuestion = $state('');

	const search = new SpotlightSearch(() => (selectedIndex = 0));

	let trimmedQuery = $derived(query.trim());
	let isTagMode = $derived(query.trimStart().startsWith('#'));
	let tagFilter = $derived(isTagMode ? query.trimStart().slice(1).trim().toLowerCase() : '');
	let isAskMode = $derived(query.trimStart().startsWith('?'));
	let askQuery = $derived(isAskMode ? query.trimStart().slice(1).trim() : '');

	let items = $derived(
		buildItems({
			askMode: isAskMode,
			tagMode: isTagMode,
			selectedTag,
			tags: tagsStore.items,
			tagFilter,
			names: search.names,
			hits: search.contents
		})
	);

	let nameCount = $derived(items.filter((i) => i.kind === 'name').length);
	let contentCount = $derived(items.filter((i) => i.kind === 'content').length);
	let canReplace = $derived(
		!isTagMode && !isAskMode && trimmedQuery.length > 0 && search.contents.length > 0
	);

	$effect(() => {
		const len = items.length;
		untrack(() => {
			if (selectedIndex > len - 1) selectedIndex = Math.max(0, len - 1);
		});
	});

	$effect(() => {
		if (isTagMode && vault.vaultPath) {
			const root = vault.vaultPath;
			untrack(() => void tagsStore.load(root));
		}
	});

	$effect(() => {
		if (isAskMode && vault.vaultPath) {
			untrack(() => void ask.ensureConfigured());
		}
	});

	function submitAsk() {
		if (!askQuery || ask.running) return;
		askedQuestion = askQuery;
		void ask.ask(askQuery);
	}

	async function openCitation(title: string) {
		if (!vault.vaultPath) return;
		const results = await searchFiles(vault.vaultPath, title);
		const match = results.find(
			(entry) => !entry.is_dir && (entry.name === `${title}.md` || entry.name === `${title}.canvas`)
		);
		if (!match) {
			toast.info(m.toast_note_not_found({ title }));
			return;
		}
		onselect(match.path);
		onclose();
	}

	function handleInput() {
		selectedTag = null;
		selectedIndex = 0;
		search.input(vault.vaultPath, query);
	}

	function activate(item: SpotlightItem) {
		if (item.kind === 'tag') {
			// Assigning `query` does not fire `oninput`, so the drill-down survives.
			selectedTag = item.tag;
			query = `#${item.tag}`;
			selectedIndex = 0;
			return;
		}
		if (item.kind === 'content') {
			onselect(item.path, trimmedQuery);
		} else {
			onselect(item.path);
		}
		onclose();
	}

	function backToTagCloud() {
		selectedTag = null;
		query = '#';
		selectedIndex = 0;
		inputEl?.focus();
	}

	function handleKeydown(e: KeyboardEvent) {
		if (e.key === 'Escape') {
			e.preventDefault();
			e.stopPropagation();
			if (ask.running) ask.cancel();
			else if (selectedTag) backToTagCloud();
			else onclose();
			return;
		}

		if (isAskMode && e.key === 'Enter') {
			e.preventDefault();
			submitAsk();
			return;
		}

		if ((e.metaKey || e.ctrlKey) && (e.key === 'h' || e.key === 'H') && !isTagMode && !isAskMode) {
			e.preventDefault();
			showReplace = !showReplace;
			return;
		}

		if (e.key === 'ArrowDown') {
			e.preventDefault();
			selectedIndex = Math.min(selectedIndex + 1, items.length - 1);
			scrollToSelected();
			return;
		}

		if (e.key === 'ArrowUp') {
			e.preventDefault();
			selectedIndex = Math.max(selectedIndex - 1, 0);
			scrollToSelected();
			return;
		}

		if (e.key === 'Enter' && items.length > 0) {
			e.preventDefault();
			const item = items[selectedIndex];
			if (item) activate(item);
		}
	}

	function scrollToSelected() {
		tick().then(() => {
			listEl
				?.querySelector(`[data-index="${selectedIndex}"]`)
				?.scrollIntoView({ block: 'nearest' });
		});
	}

	async function handleReplaceInFile(path: string) {
		if (!canReplace) return;
		replacing = true;
		try {
			if (await replaceInOneFile(path, trimmedQuery, replaceQuery)) {
				await search.rerun(vault.vaultPath, query);
			}
		} finally {
			replacing = false;
		}
	}

	async function handleReplaceAll() {
		if (!canReplace) return;
		replacing = true;
		try {
			const paths = search.contents.map((hit) => hit.path);
			if (await replaceInEveryFile(paths, trimmedQuery, replaceQuery)) {
				await search.rerun(vault.vaultPath, query);
			}
		} finally {
			replacing = false;
		}
	}

	onMount(() => {
		inputEl?.focus();
	});

	onDestroy(() => {
		search.dispose();
	});
</script>

<!-- svelte-ignore a11y_no_static_element_interactions -->
<!-- svelte-ignore a11y_click_events_have_key_events -->
<div
	class="fixed inset-0 z-200 flex justify-center bg-(--color-bg-overlay) pt-[min(18vh,120px)]"
	onclick={onclose}
	onkeydown={handleKeydown}
>
	<div
		class="flex max-h-130 w-[min(640px,calc(100vw-32px))] animate-in flex-col self-start overflow-hidden rounded-xl border border-border bg-background shadow-(--shadow-lg) duration-120 ease-out fade-in-0 slide-in-from-top-[6px]"
		role="dialog"
		aria-modal="true"
		tabindex="-1"
		aria-label={m.spotlight_title()}
		onclick={(e) => e.stopPropagation()}
	>
		<div class="flex items-center gap-2.5 border-b border-border px-4 py-3">
			{#if search.searching || ask.running}
				<Loader size={16} class="shrink-0 animate-spin text-subtle-foreground" />
			{:else if isAskMode}
				<Sparkles size={16} class="shrink-0 text-subtle-foreground" />
			{:else}
				<Search size={16} class="shrink-0 text-subtle-foreground" />
			{/if}
			<input
				bind:this={inputEl}
				bind:value={query}
				oninput={handleInput}
				placeholder={isAskMode ? m.spotlight_ask_placeholder() : m.spotlight_placeholder()}
				type="text"
				spellcheck="false"
				autocomplete="off"
				class="min-w-0 flex-1 border-none bg-transparent p-0 text-base text-foreground caret-(--color-bg-brand) shadow-none outline-none placeholder:text-subtle-foreground"
			/>
			{#if !isTagMode && !isAskMode}
				<button
					type="button"
					onclick={() => (showReplace = !showReplace)}
					title={m.spotlight_toggle_replace()}
					aria-pressed={showReplace}
					class="flex size-6 shrink-0 items-center justify-center rounded-xs transition-colors hover:bg-surface-3 hover:text-foreground {showReplace
						? 'bg-accent text-accent-foreground'
						: 'text-subtle-foreground'}"
				>
					<Replace size={14} />
				</button>
			{/if}
			<kbd
				class="shrink-0 rounded-xs border border-border px-1.5 py-0.5 font-mono text-xs text-subtle-foreground"
				>esc</kbd
			>
		</div>

		{#if showReplace && !isTagMode && !isAskMode}
			<SpotlightReplaceRow
				bind:replaceQuery
				{canReplace}
				{replacing}
				count={search.contents.length}
				onreplaceall={handleReplaceAll}
			/>
		{/if}

		{#if isTagMode && selectedTag}
			<SpotlightTagCrumb tag={selectedTag} count={items.length} onback={backToTagCloud} />
		{/if}

		{#if isAskMode}
			<SpotlightAsk {askedQuestion} {onsettings} oncite={openCitation} />
		{:else if items.length > 0}
			<div class="min-h-0 flex-1 overflow-y-auto p-1.5" role="listbox" bind:this={listEl}>
				{#each items as item, i (itemKey(item))}
					{@const prev = items[i - 1]}

					{#if item.kind === 'name' && prev?.kind !== 'name'}
						<div
							class="px-2.5 pt-2 pb-1 text-xs font-semibold tracking-wide text-subtle-foreground uppercase"
						>
							{m.spotlight_group_names()} · {nameCount}
						</div>
					{:else if item.kind === 'content' && prev?.kind !== 'content'}
						<div
							class="mt-1 border-t border-border px-2.5 pt-3 pb-1 text-xs font-semibold tracking-wide text-subtle-foreground uppercase"
						>
							{m.spotlight_group_contents()} · {contentCount}
						</div>
					{:else if item.kind === 'tag' && prev?.kind !== 'tag'}
						<div
							class="px-2.5 pt-2 pb-1 text-xs font-semibold tracking-wide text-subtle-foreground uppercase"
						>
							{m.spotlight_group_tags()}
						</div>
					{/if}

					<SpotlightRow
						{item}
						index={i}
						selected={i === selectedIndex}
						query={trimmedQuery}
						vaultPath={vault.vaultPath}
						{showReplace}
						onactivate={() => activate(item)}
						onhover={() => (selectedIndex = i)}
						onreplace={handleReplaceInFile}
					/>
				{/each}
			</div>
		{:else if isTagMode && tagsStore.loading}
			<div class="px-4 py-8 text-center text-sm text-subtle-foreground">{m.tags_loading()}</div>
		{:else if isTagMode && selectedTag}
			<div class="px-4 py-8 text-center text-sm text-subtle-foreground">{m.tags_no_files()}</div>
		{:else if isTagMode}
			<div class="px-4 py-8 text-center text-sm text-subtle-foreground">{m.tags_empty()}</div>
		{:else if trimmedQuery && !search.searching}
			<div class="px-4 py-8 text-center text-sm text-subtle-foreground">
				{m.spotlight_no_results()}
			</div>
		{:else if !trimmedQuery}
			<div class="px-4 py-8 text-center text-sm text-subtle-foreground">{m.spotlight_hint()}</div>
		{/if}
	</div>
</div>
