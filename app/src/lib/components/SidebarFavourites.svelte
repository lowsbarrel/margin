<script lang="ts">
	import { files } from '$lib/stores/files.svelte';
	import { favourites } from '$lib/stores/favourites.svelte';
	import { vault } from '$lib/stores/vault.svelte';
	import { Star } from '@lucide/svelte';
	import * as m from '$lib/paraglide/messages.js';
	import { displayPath, displayName } from '$lib/utils/sidebar-ops';
	import type { MenuTarget } from '$lib/utils/sidebar-menu';

	interface Props {
		onfileselect: (path: string) => void;
		oncontextmenu: (target: MenuTarget, x: number, y: number) => void;
	}

	let { onfileselect, oncontextmenu }: Props = $props();
</script>

<!-- Layout and row styling come from the shared panel/list classes in
     `$lib/styles/components.css`; this component adds no CSS of its own. -->
<div class="panel-header">
	<span class="panel-title">{m.sidebar_favourites()}</span>
</div>

<div class="panel-content">
	{#if favourites.list.length === 0}
		<div class="empty-note">{m.sidebar_no_favourites()}</div>
	{/if}

	{#each favourites.list as favPath (favPath)}
		{@const name = favPath.split('/').pop() ?? favPath}
		<button
			class="list-row"
			class:is-active={files.activeFile === favPath}
			onclick={() => onfileselect(favPath)}
			oncontextmenu={(event) => {
				event.preventDefault();
				oncontextmenu(
					{ kind: 'entry', entry: { path: favPath, name, is_dir: false, modified: 0 } },
					event.clientX,
					event.clientY
				);
			}}
		>
			<Star size={14} class="icon-brand" />
			<div class="list-row-text">
				<span class="list-row-name">{displayName(name)}</span>
				{#if displayPath(favPath, vault.vaultPath)}
					<span class="list-row-meta">{displayPath(favPath, vault.vaultPath)}</span>
				{/if}
			</div>
		</button>
	{/each}
</div>
