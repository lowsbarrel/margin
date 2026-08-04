<script lang="ts">
	/**
	 * Spotlight — the single search surface for the vault.
	 *
	 * Replaces both the old QuickSwitcher (note names only) and the sidebar
	 * Search view (contents + tags + replace-across-files). One palette now
	 * covers all three:
	 *
	 *   • plain text  → note names (trie) + note contents (FTS5), two groups
	 *   • `#…`        → tag browser: tag cloud, then the files carrying that tag
	 *   • replace row → replace-across-files over the current content hits
	 */
	import { onMount, onDestroy, tick, untrack } from 'svelte';
	import { vault } from '$lib/stores/vault.svelte';
	import {
		searchFiles,
		searchIndex,
		replaceInFile,
		type FsEntry,
		type SearchHit
	} from '$lib/fs/bridge';
	import { tags as tagsStore } from '$lib/stores/tags.svelte';
	import { toast } from '$lib/stores/toast.svelte';
	import {
		FileText,
		Image,
		FileType,
		Hash,
		Search,
		Replace,
		ReplaceAll,
		ChevronLeft,
		Loader
	} from '@lucide/svelte';
	import { IMAGE_EXTS } from '$lib/utils/mime';
	import { displayName } from '$lib/utils/filename';
	import { displayPath, splitHighlight } from '$lib/utils/sidebar-ops';
	import * as m from '$lib/paraglide/messages.js';

	interface Props {
		/** Open a note. `searchText` scrolls the editor to the matched excerpt. */
		onselect: (path: string, searchText?: string) => void;
		onclose: () => void;
	}

	let { onselect, onclose }: Props = $props();

	/** How many filename hits to surface before the content group starts. */
	const MAX_NAME_RESULTS = 8;
	/** Content hits fetched — also the working set for "replace in all files". */
	const MAX_CONTENT_RESULTS = 100;
	const DEBOUNCE_MS = 90;

	let query = $state('');
	let nameResults = $state<FsEntry[]>([]);
	let contentResults = $state<SearchHit[]>([]);
	let selectedIndex = $state(0);
	let searching = $state(false);
	let inputEl = $state<HTMLInputElement | null>(null);
	let listEl = $state<HTMLDivElement | null>(null);

	let debounceTimer: ReturnType<typeof setTimeout> | null = null;
	/**
	 * Monotonic ticket for in-flight searches. Every keystroke takes a new one;
	 * a response whose ticket is stale is dropped, so a slow content query can
	 * never land on top of a newer, faster one.
	 */
	let searchGeneration = 0;

	// ── Tag mode ──
	let allTags = $derived(tagsStore.items);
	let tagsLoading = $derived(tagsStore.loading);
	/** Set once the user drills into a tag; null means "showing the tag cloud". */
	let selectedTag = $state<string | null>(null);

	// ── Replace across files ──
	let showReplace = $state(false);
	let replaceQuery = $state('');
	let replacing = $state(false);

	let trimmedQuery = $derived(query.trim());
	let isTagMode = $derived(query.trimStart().startsWith('#'));
	let tagFilter = $derived(isTagMode ? query.trimStart().slice(1).trim().toLowerCase() : '');

	// ── Flattened item list ───────────────────────────────────────────────
	// Keyboard nav runs over one flat array regardless of mode; the template
	// draws a group header wherever the kind changes.

	type Item =
		| { kind: 'name'; path: string; entry: FsEntry }
		| { kind: 'content'; path: string; hit: SearchHit }
		| { kind: 'tag'; tag: string; count: number }
		| { kind: 'tagfile'; path: string; tag: string };

	let items = $derived.by((): Item[] => {
		if (isTagMode) {
			if (selectedTag) {
				const tag = selectedTag;
				const paths = allTags.find((t) => t.tag === tag)?.files ?? [];
				return paths.map((path) => ({ kind: 'tagfile', path, tag }));
			}
			return allTags
				.filter((t) => !tagFilter || t.tag.includes(tagFilter))
				.map((t) => ({ kind: 'tag', tag: t.tag, count: t.count }));
		}

		const out: Item[] = [];
		const named = nameResults.map((entry) => entry.path);
		for (const entry of nameResults) out.push({ kind: 'name', path: entry.path, entry });
		for (const hit of contentResults) {
			// A note whose name matched is already listed above; repeating it in
			// the content group would fill the palette with duplicates (the index
			// weights note names 10x, so almost every name hit is also a content
			// hit). `named` holds at most MAX_NAME_RESULTS entries, so the linear
			// scan is cheaper than the Set it replaces.
			if (named.includes(hit.path)) continue;
			out.push({ kind: 'content', path: hit.path, hit });
		}
		return out;
	});

	let nameCount = $derived(items.filter((i) => i.kind === 'name').length);
	let contentCount = $derived(items.filter((i) => i.kind === 'content').length);
	let canReplace = $derived(!isTagMode && trimmedQuery.length > 0 && contentResults.length > 0);

	// Keep the cursor inside the list when results shrink under it.
	$effect(() => {
		const len = items.length;
		untrack(() => {
			if (selectedIndex > len - 1) selectedIndex = Math.max(0, len - 1);
		});
	});

	// The tag list is loaded lazily — only someone who types `#` pays for it.
	$effect(() => {
		if (isTagMode && vault.vaultPath) {
			const root = vault.vaultPath;
			untrack(() => void tagsStore.load(root));
		}
	});

	function getIcon(path: string) {
		const ext = path.slice(path.lastIndexOf('.')).toLowerCase();
		if (ext === '.md') return FileText;
		if (IMAGE_EXTS.has(ext)) return Image;
		if (ext === '.pdf') return FileType;
		return Hash;
	}

	// ── Search ────────────────────────────────────────────────────────────

	async function runSearch(q: string) {
		const gen = ++searchGeneration;
		const trimmed = q.trim();

		if (!vault.vaultPath || !trimmed || trimmed.startsWith('#')) {
			nameResults = [];
			contentResults = [];
			searching = false;
			return;
		}

		searching = true;
		try {
			// Names come from the in-memory trie, contents from the FTS5 index.
			// Issued together so the palette isn't gated on the slower of the two.
			const [names, hits] = await Promise.all([
				searchFiles(vault.vaultPath, trimmed),
				searchIndex(vault.vaultPath, trimmed, MAX_CONTENT_RESULTS).catch(() => [] as SearchHit[])
			]);
			if (gen !== searchGeneration) return;
			// `searchFiles` is already ranked best-first by the Rust trie
			// (name prefix → token prefix → substring → path → fuzzy); do not re-sort.
			nameResults = names.slice(0, MAX_NAME_RESULTS);
			contentResults = hits;
			selectedIndex = 0;
		} catch (err) {
			if (gen !== searchGeneration) return;
			console.warn('Spotlight search failed:', err);
			nameResults = [];
			contentResults = [];
		} finally {
			if (gen === searchGeneration) searching = false;
		}
	}

	function handleInput() {
		selectedTag = null;
		selectedIndex = 0;
		if (debounceTimer) clearTimeout(debounceTimer);

		// Tag mode reads a list we already hold in memory — no round trip, so no
		// debounce. Bump the generation anyway to void any content search still
		// in flight from before the `#`.
		if (query.trimStart().startsWith('#') || !query.trim()) {
			searchGeneration++;
			nameResults = [];
			contentResults = [];
			searching = false;
			return;
		}

		searching = true;
		const q = query;
		debounceTimer = setTimeout(() => runSearch(q), DEBOUNCE_MS);
	}

	// ── Activation ────────────────────────────────────────────────────────

	function activate(item: Item) {
		if (item.kind === 'tag') {
			// Drill into the tag. Assigning `query` does not fire `oninput`, so
			// `selectedTag` survives.
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

	// ── Keyboard ──────────────────────────────────────────────────────────

	function handleKeydown(e: KeyboardEvent) {
		if (e.key === 'Escape') {
			e.preventDefault();
			e.stopPropagation();
			// Escape steps out of a tag before it closes the palette.
			if (selectedTag) backToTagCloud();
			else onclose();
			return;
		}

		if ((e.metaKey || e.ctrlKey) && (e.key === 'h' || e.key === 'H') && !isTagMode) {
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
			return;
		}
	}

	function scrollToSelected() {
		tick().then(() => {
			listEl
				?.querySelector(`[data-index="${selectedIndex}"]`)
				?.scrollIntoView({ block: 'nearest' });
		});
	}

	// ── Replace across files ──────────────────────────────────────────────

	async function handleReplaceInFile(path: string) {
		if (!canReplace) return;
		replacing = true;
		try {
			const count = await replaceInFile(path, trimmedQuery, replaceQuery, false);
			if (count > 0) {
				toast.success(m.toast_replaced({ count: String(count) }));
				await runSearch(query);
			}
		} catch (err) {
			toast.error(m.toast_replace_failed({ error: String(err) }));
		} finally {
			replacing = false;
		}
	}

	async function handleReplaceAll() {
		if (!canReplace) return;
		replacing = true;
		const paths = contentResults.map((h) => h.path);
		let total = 0;
		let failed = 0;
		try {
			for (const path of paths) {
				try {
					total += await replaceInFile(path, trimmedQuery, replaceQuery, false);
				} catch (err) {
					failed += 1;
					console.warn(`Replace in ${path} failed:`, err);
				}
			}
			if (total > 0) {
				toast.success(
					m.toast_replaced_in_files({ count: String(total), files: String(paths.length) })
				);
				await runSearch(query);
			}
			if (failed > 0) toast.error(m.toast_replace_failed({ error: String(failed) }));
		} finally {
			replacing = false;
		}
	}

	onMount(() => {
		inputEl?.focus();
	});

	onDestroy(() => {
		if (debounceTimer) clearTimeout(debounceTimer);
	});
</script>

<!-- svelte-ignore a11y_no_static_element_interactions -->
<!-- svelte-ignore a11y_click_events_have_key_events -->
<div
	class="fixed inset-0 z-[200] flex justify-center bg-[var(--color-bg-overlay)] pt-[min(18vh,120px)]"
	onclick={onclose}
	onkeydown={handleKeydown}
>
	<div
		class="flex max-h-[520px] w-[min(640px,calc(100vw_-_32px))] animate-in flex-col self-start overflow-hidden rounded-xl border border-border bg-background shadow-[var(--shadow-lg)] duration-120 ease-out fade-in-0 slide-in-from-top-[6px]"
		role="dialog"
		aria-modal="true"
		tabindex="-1"
		aria-label={m.spotlight_title()}
		onclick={(e) => e.stopPropagation()}
	>
		<!-- Query row -->
		<div class="flex items-center gap-2.5 border-b border-border px-4 py-3">
			{#if searching}
				<Loader size={16} class="shrink-0 animate-spin text-subtle-foreground" />
			{:else}
				<Search size={16} class="shrink-0 text-subtle-foreground" />
			{/if}
			<input
				bind:this={inputEl}
				bind:value={query}
				oninput={handleInput}
				placeholder={m.spotlight_placeholder()}
				type="text"
				spellcheck="false"
				autocomplete="off"
				class="min-w-0 flex-1 border-none bg-transparent p-0 text-base text-foreground caret-[var(--color-bg-brand)] shadow-none outline-none placeholder:text-subtle-foreground"
			/>
			{#if !isTagMode}
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

		<!-- Replace row -->
		{#if showReplace && !isTagMode}
			<div class="flex items-center gap-2.5 border-b border-border px-4 py-2.5">
				<Replace size={14} class="shrink-0 text-subtle-foreground" />
				<input
					bind:value={replaceQuery}
					placeholder={m.spotlight_replace_placeholder()}
					type="text"
					spellcheck="false"
					autocomplete="off"
					class="min-w-0 flex-1 border-none bg-transparent p-0 text-sm text-foreground caret-[var(--color-bg-brand)] shadow-none outline-none placeholder:text-subtle-foreground"
				/>
				<button
					type="button"
					onclick={handleReplaceAll}
					disabled={!canReplace || replacing}
					title={m.spotlight_replace_all_files()}
					class="flex shrink-0 items-center gap-1.5 rounded-xs border border-border px-2 py-1 text-xs text-muted-foreground transition-colors hover:bg-surface-3 hover:text-foreground disabled:cursor-not-allowed disabled:opacity-40 disabled:hover:bg-transparent"
				>
					<ReplaceAll size={13} />
					{m.spotlight_replace_all_short({ count: String(contentResults.length) })}
				</button>
			</div>
		{/if}

		<!-- Tag drill-down breadcrumb -->
		{#if isTagMode && selectedTag}
			<div class="flex items-center justify-between border-b border-border px-4 py-2">
				<button
					type="button"
					onclick={backToTagCloud}
					class="flex items-center gap-1.5 text-xs text-muted-foreground transition-colors hover:text-foreground"
				>
					<ChevronLeft size={13} />
					<span class="font-mono text-accent-foreground">#{selectedTag}</span>
				</button>
				<span class="text-xs text-subtle-foreground">
					{items.length}
					{m.tags_files({ count: items.length })}
				</span>
			</div>
		{/if}

		<!-- Results -->
		{#if items.length > 0}
			<div class="min-h-0 flex-1 overflow-y-auto p-1.5" role="listbox" bind:this={listEl}>
				{#each items as item, i (item.kind + ':' + (item.kind === 'tag' ? item.tag : item.path))}
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

					<!-- svelte-ignore a11y_click_events_have_key_events -->
					<div
						data-index={i}
						role="option"
						tabindex="-1"
						aria-selected={i === selectedIndex}
						onclick={() => activate(item)}
						onmouseenter={() => (selectedIndex = i)}
						class="relative flex cursor-pointer items-center gap-2.5 rounded-lg px-2.5 py-1.5 transition-colors {i ===
						selectedIndex
							? 'bg-accent'
							: ''}"
					>
						{#if i === selectedIndex}
							<span class="absolute top-2 bottom-2 left-0 w-0.5 rounded-full bg-brand"></span>
						{/if}

						{#if item.kind === 'tag'}
							<Hash
								size={16}
								class="shrink-0 {i === selectedIndex
									? 'text-accent-foreground'
									: 'text-subtle-foreground'}"
							/>
							<span class="min-w-0 truncate font-mono text-sm text-foreground">#{item.tag}</span>
							<span class="ml-auto shrink-0 text-xs text-subtle-foreground">{item.count}</span>
						{:else}
							{@const Icon = item.kind === 'name' ? getIcon(item.path) : FileText}
							{@const name =
								item.kind === 'name' ? item.entry.name : (item.path.split('/').pop() ?? item.path)}
							{@const folder = displayPath(item.path, vault.vaultPath)}
							<Icon
								size={16}
								class="shrink-0 self-start {i === selectedIndex
									? 'text-accent-foreground'
									: 'text-subtle-foreground'} mt-0.5"
							/>
							<div class="flex min-w-0 flex-1 flex-col gap-0.5">
								<div class="flex min-w-0 items-baseline gap-2">
									<span class="min-w-0 truncate text-sm font-medium text-foreground"
										>{displayName(name)}</span
									>
									{#if folder}
										<span class="ml-auto min-w-0 shrink truncate text-xs text-subtle-foreground"
											>{folder}</span
										>
									{/if}
								</div>
								{#if item.kind === 'content' && item.hit.snippet}
									<span class="min-w-0 truncate text-xs text-muted-foreground">
										{#each splitHighlight(item.hit.snippet, trimmedQuery) as seg, si (si)}{#if seg.match}<mark
													class="rounded-xs bg-[var(--color-brand-32)] px-0.5 font-medium text-foreground"
													>{seg.text}</mark
												>{:else}{seg.text}{/if}{/each}
									</span>
								{/if}
							</div>
							{#if item.kind === 'content' && showReplace}
								<!-- svelte-ignore a11y_click_events_have_key_events -->
								<span
									role="button"
									tabindex="-1"
									onclick={(e) => {
										e.stopPropagation();
										handleReplaceInFile(item.path);
									}}
									title={m.spotlight_replace_all_in_file()}
									class="flex size-6 shrink-0 items-center justify-center self-center rounded-xs text-subtle-foreground transition-colors hover:bg-surface-3 hover:text-foreground"
								>
									<Replace size={12} />
								</span>
							{/if}
						{/if}
					</div>
				{/each}
			</div>
		{:else if isTagMode && tagsLoading}
			<div class="px-4 py-8 text-center text-sm text-subtle-foreground">{m.tags_loading()}</div>
		{:else if isTagMode && selectedTag}
			<div class="px-4 py-8 text-center text-sm text-subtle-foreground">{m.tags_no_files()}</div>
		{:else if isTagMode}
			<div class="px-4 py-8 text-center text-sm text-subtle-foreground">{m.tags_empty()}</div>
		{:else if trimmedQuery && !searching}
			<div class="px-4 py-8 text-center text-sm text-subtle-foreground">
				{m.spotlight_no_results()}
			</div>
		{:else if !trimmedQuery}
			<div class="px-4 py-8 text-center text-sm text-subtle-foreground">{m.spotlight_hint()}</div>
		{/if}
	</div>
</div>
