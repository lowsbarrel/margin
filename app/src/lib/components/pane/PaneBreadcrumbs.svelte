<script lang="ts">
	import { tick, untrack } from 'svelte';
	import { ChevronRight, FileText, Folder, FolderOpen } from '@lucide/svelte';
	import * as m from '$lib/paraglide/messages.js';
	import { vault } from '$lib/stores/vault.svelte';
	import { displayPath } from '$lib/utils/sidebar-ops';
	import { vaultBreadcrumbs, type Breadcrumb } from '$lib/utils/breadcrumbs';
	import { BreadcrumbMenu } from './breadcrumb-menu.svelte';

	interface Props {
		paneIndex: number;
		path: string;
	}

	let { paneIndex, path }: Props = $props();

	const MENU_WIDTH = 288;

	let navEl = $state<HTMLElement | null>(null);
	let stripEl = $state<HTMLDivElement | null>(null);
	let listEl = $state<HTMLDivElement | null>(null);
	let left = $state(0);
	let top = $state(0);
	let maxHeight = $state(0);

	const menu = new BreadcrumbMenu();
	const crumbs = $derived(vaultBreadcrumbs(path, vault.vaultPath));

	$effect(() => {
		const current = path;
		untrack(() => {
			if (menu.source && menu.source !== current) menu.close();
		});
	});

	async function place() {
		await tick();
		const anchor =
			menu.index === null
				? null
				: stripEl?.querySelector<HTMLElement>(`[data-crumb="${menu.index}"]`);
		if (!anchor) return;
		const rect = anchor.getBoundingClientRect();
		left = Math.max(8, Math.min(rect.left, window.innerWidth - MENU_WIDTH - 8));
		top = rect.bottom + 4;
		maxHeight = Math.max(140, window.innerHeight - top - 12);
	}

	function focusAnchor(restore: boolean) {
		const index = menu.index;
		menu.close();
		if (restore && index !== null) {
			stripEl?.querySelector<HTMLElement>(`[data-crumb="${index}"]`)?.focus();
		}
	}

	async function toggle(index: number, crumb: Breadcrumb) {
		if (menu.index === index) return focusAnchor(false);
		const listPath = crumb.isDir ? crumb.path : crumb.path.slice(0, crumb.path.lastIndexOf('/'));
		await menu.show(index, listPath, path);
		await place();
		listEl?.focus();
	}

	function onDocumentMouseDown(event: MouseEvent) {
		if (!menu.open) return;
		const target = event.target as Node;
		if (navEl?.contains(target) || listEl?.contains(target)) return;
		menu.close();
	}

	async function onKeydown(event: KeyboardEvent) {
		if (event.key === 'Escape') {
			event.preventDefault();
			event.stopPropagation();
			return focusAnchor(true);
		}
		if (event.key === 'ArrowDown' || event.key === 'ArrowUp') {
			event.preventDefault();
			return menu.moveFocus(event.key === 'ArrowDown' ? 1 : -1);
		}
		if (event.key === 'ArrowRight') {
			event.preventDefault();
			return menu.right();
		}
		if (event.key === 'ArrowLeft') {
			event.preventDefault();
			return menu.left();
		}
		if (event.key === 'Enter') {
			event.preventDefault();
			return menu.activate(paneIndex);
		}
		if (event.key === 'Backspace') {
			event.preventDefault();
			return menu.backspace();
		}
		if (event.key.length === 1 && !event.metaKey && !event.ctrlKey && !event.altKey) {
			event.preventDefault();
			return menu.type(event.key);
		}
	}

	function rowName(row: { name: string }) {
		return row.name.replace(/\.(md|canvas)$/, '');
	}

	function rowClass(active: boolean, focused: boolean) {
		const base =
			'breadcrumb-row relative flex w-full cursor-pointer items-center gap-2 rounded-sm py-1.5 pr-2.5 text-left text-sm tracking-normal transition-colors';
		const row = active
			? `${base} bg-accent font-medium text-foreground [&_svg]:text-accent-foreground before:absolute before:top-1.5 before:bottom-1.5 before:left-0 before:w-0.5 before:rounded-full before:bg-brand before:content-['']`
			: focused
				? `${base} bg-surface-3 text-foreground`
				: `${base} font-normal text-muted-foreground hover:bg-surface-3 hover:text-foreground`;
		return focused ? `${row} is-focused` : row;
	}
</script>

<svelte:window onresize={place} />
<svelte:document onmousedown={onDocumentMouseDown} />

{#if crumbs.length > 0}
	<nav
		class="flex min-h-7.5 items-center border-b border-border bg-background text-xs whitespace-nowrap text-subtle-foreground"
		aria-label={m.breadcrumb_nav_label()}
		bind:this={navEl}
	>
		<div
			class="flex flex-1 scrollbar-none items-center overflow-x-auto px-4 py-1.5"
			bind:this={stripEl}
			onscroll={place}
		>
			{#each crumbs as crumb, i (crumb.path)}
				{#if i > 0}
					<ChevronRight size={12} class="shrink-0" />
				{/if}
				<button
					type="button"
					class="breadcrumb-segment flex shrink-0 cursor-pointer items-center rounded-sm px-1 py-0.5 transition-colors hover:bg-surface-3 hover:text-foreground {menu.index ===
					i
						? 'bg-surface-3 text-foreground'
						: i === crumbs.length - 1
							? 'font-medium text-muted-foreground'
							: ''}"
					data-crumb={i}
					aria-haspopup="tree"
					aria-label={m.breadcrumb_browse({ name: crumb.label })}
					aria-expanded={menu.index === i}
					onclick={() => toggle(i, crumb)}
				>
					{crumb.label}
				</button>
			{/each}
		</div>
	</nav>
{/if}

{#if menu.open}
	<div
		class="surface-popover fixed z-200 flex flex-col p-1"
		style:left="{left}px"
		style:top="{top}px"
		style:max-height="{maxHeight}px"
		style:width="{MENU_WIDTH}px"
	>
		<div
			class="flex min-h-6 shrink-0 items-center gap-2 border-b border-border px-2.5 pb-1.5 text-xs"
		>
			<span class="min-w-0 flex-1 truncate text-subtle-foreground">{menu.title}</span>
			{#if menu.filtering}
				<span class="shrink-0 font-mono text-muted-foreground">{menu.filter}</span>
			{/if}
		</div>
		<div
			class="min-h-0 flex-1 overflow-y-auto py-1 outline-none"
			role="tree"
			tabindex="-1"
			aria-label={m.breadcrumb_folder_contents({ name: menu.title })}
			aria-activedescendant={menu.visible.length > 0 ? `breadcrumb-row-${menu.focus}` : undefined}
			bind:this={listEl}
			onkeydown={onKeydown}
		>
			{#each menu.visible as row, i (row.path)}
				{@const active = !row.is_dir && row.path === path}
				{@const open = row.is_dir && menu.expanded.has(row.path)}
				{@const folder = menu.filtering ? displayPath(row.path, vault.vaultPath) : ''}
				<button
					type="button"
					id="breadcrumb-row-{i}"
					role="treeitem"
					tabindex="-1"
					aria-level={row.depth + 1}
					aria-selected={active}
					aria-expanded={row.is_dir ? open : undefined}
					class={rowClass(active, i === menu.focus)}
					style="padding-left: {row.depth * 14 + 6}px;"
					onmouseenter={() => (menu.focus = i)}
					onclick={() => {
						menu.focus = i;
						return menu.activate(paneIndex);
					}}
				>
					<span class="flex size-3.5 shrink-0 items-center justify-center">
						{#if row.is_dir}
							<span class="flex items-center transition-transform {open ? 'rotate-90' : ''}">
								<ChevronRight size={12} />
							</span>
						{/if}
					</span>
					{#if open}
						<FolderOpen size={16} class="shrink-0" />
					{:else if row.is_dir}
						<Folder size={16} class="shrink-0" />
					{:else}
						<FileText size={16} class="shrink-0" />
					{/if}
					<span class="min-w-0 flex-1 truncate">{rowName(row)}</span>
					{#if folder}
						<span class="shrink-0 text-xs text-subtle-foreground">{folder}</span>
					{/if}
				</button>
			{:else}
				<p class="px-2.5 py-2 text-sm text-subtle-foreground" role="none">
					{menu.filtering ? m.breadcrumb_no_matches() : m.breadcrumb_empty()}
				</p>
			{/each}
		</div>
		{#if !menu.filtering}
			<p class="shrink-0 border-t border-border px-2.5 pt-1.5 text-xs text-subtle-foreground">
				{m.breadcrumb_filter_hint()}
			</p>
		{/if}
	</div>
{/if}

<style>
	.breadcrumb-row.is-focused {
		outline: 2px solid var(--color-border-focus);
		outline-offset: -2px;
	}

	.breadcrumb-segment:focus-visible {
		outline: 2px solid var(--color-border-focus);
		outline-offset: -2px;
	}
</style>
