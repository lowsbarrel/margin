<script lang="ts">
	import { ArrowDownAZ, ArrowDownWideNarrow, ChevronsDownUp } from '@lucide/svelte';
	import { FilePlus, FolderPlus, PenLine } from '$lib/components/movingicons';
	import { files } from '$lib/stores/files.svelte';
	import { handleNewFolder, handleNewNote } from '$lib/utils/page-actions';
	import { IconButton } from '$lib/ui';
	import * as m from '$lib/paraglide/messages.js';
</script>

<div class="panel-header">
	<span class="panel-title">{m.sidebar_explorer()}</span>
	<div class="panel-actions">
		<IconButton
			icon={files.sortOrder === 'name' ? ArrowDownAZ : ArrowDownWideNarrow}
			size="sm"
			onclick={() => files.toggleSortOrder()}
			title={files.sortOrder === 'name' ? m.sidebar_sort_by_date() : m.sidebar_sort_by_name()}
		/>
		<IconButton
			icon={ChevronsDownUp}
			size="sm"
			onclick={() => files.collapseAll()}
			title={m.sidebar_collapse_all()}
		/>
		<IconButton
			icon={FilePlus}
			size="sm"
			onclick={() => handleNewNote()}
			title={m.sidebar_new_file()}
		/>
		<IconButton
			icon={PenLine}
			size="sm"
			onclick={() => handleNewNote(undefined, 'Untitled.canvas')}
			title={m.sidebar_new_canvas()}
		/>
		<IconButton
			icon={FolderPlus}
			size="sm"
			onclick={() => handleNewFolder()}
			title={m.sidebar_new_folder()}
		/>
	</div>
</div>
