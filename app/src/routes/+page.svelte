<script lang="ts">
	import Login from '$lib/components/Login.svelte';
	import Sidebar from '$lib/components/Sidebar.svelte';
	import PaneView from '$lib/components/PaneView.svelte';
	import StatusBar from '$lib/components/StatusBar.svelte';
	import Settings from '$lib/components/Settings.svelte';
	import HistoryPanel from '$lib/components/HistoryPanel.svelte';
	import TrashDialog from '$lib/components/TrashDialog.svelte';
	import BacklinksPanel from '$lib/components/BacklinksPanel.svelte';
	import TerminalPanel from '$lib/components/TerminalPanel.svelte';

	import Spotlight from '$lib/components/Spotlight.svelte';
	import DragPreview from '$lib/components/shell/DragPreview.svelte';
	import TabContextMenu from '$lib/components/shell/TabContextMenu.svelte';
	import { Toaster } from '$lib/ui';
	import { vault } from '$lib/stores/vault.svelte';
	import { files } from '$lib/stores/files.svelte';
	import { editor } from '$lib/stores/editor.svelte';
	import { panes } from '$lib/stores/panes.svelte';
	import { terminals } from '$lib/stores/terminals.svelte';
	import { drag } from '$lib/stores/drag.svelte';
	import {
		handleRename,
		handleDelete,
		handleNewNote,
		handleWikiLink,
		handleLogout
	} from '$lib/utils/page-actions';
	import { executeDrop, executeStripDrop, startDividerDrag } from '$lib/utils/tab-drag';
	import { runManualSync } from '$lib/sync/s3sync';
	import {
		shellLayout,
		scheduleWorkspaceSave,
		saveWorkspace,
		forgetWorkspaceRestore
	} from '$lib/components/shell/workspace-persistence.svelte';
	import { vaultBootstrap, initVaultBootstrap } from '$lib/components/shell/vault-bootstrap.svelte';
	import {
		initPendingScroll,
		scrollEditorToText
	} from '$lib/components/shell/editor-scroll.svelte';
	import { handleGlobalKeydown } from '$lib/components/shell/keyboard-shortcuts';
	import { initAppLifecycle } from '$lib/components/shell/app-lifecycle.svelte';

	let showSettings = $state(false);
	let showSpotlight = $state(false);
	let showHistory = $state(false);
	let showTrash = $state(false);
	let showBacklinks = $state(false);
	let panesContainerEl = $state<HTMLElement | null>(null);
	let dividerResizing = $state(false);
	let dropTarget = $state<{
		paneIndex: number;
		zone: 'left' | 'center' | 'right';
	} | null>(null);
	let tabContextMenu = $state<{
		x: number;
		y: number;
		paneIndex: number;
		tabIndex: number;
	} | null>(null);

	const shortcutActions = {
		toggleTerminal: () => terminals.toggle(),
		toggleSpotlight: () => (showSpotlight = !showSpotlight),
		openSpotlight: () => (showSpotlight = true),
		reopenClosedTab: () => panes.reopenClosedTab(),
		toggleSidebar: () => (shellLayout.sidebarOpen = !shellLayout.sidebarOpen),
		newNote: () => handleNewNote(),
		closeTab: () => {
			const pane = panes.activePane;
			if (pane.activeTabIndex >= 0) panes.closeTab(panes.activePaneIndex, pane.activeTabIndex);
		},
		toggleViewMode
	};

	async function handleFileSelect(path: string, searchText?: string) {
		const opened = await panes.openFile(path);
		if (opened && searchText) scrollEditorToText(searchText);
	}

	function toggleViewMode() {
		const tab = panes.activeTab;
		if (!tab || tab.type !== 'markdown') return;
		tab.viewMode = tab.viewMode === 'source' ? 'rich' : 'source';
	}

	function handleTabContextMenu(e: MouseEvent, paneIndex: number, tabIndex: number) {
		e.preventDefault();
		e.stopPropagation();
		tabContextMenu = { x: e.clientX, y: e.clientY, paneIndex, tabIndex };
	}

	function onLogout() {
		handleLogout(() => {
			saveWorkspace();
			forgetWorkspaceRestore();
		});
	}

	initAppLifecycle();
	initPendingScroll();
	initVaultBootstrap();

	$effect(() => {
		const _panes = panes.list.map((p) => ({
			tabs: p.tabs.map((t) => [t.path, t.viewMode]),
			activeTabIndex: p.activeTabIndex
		}));
		const _flexes = panes.flexes;
		const _activePane = panes.activePaneIndex;
		const _sidebarOpen = shellLayout.sidebarOpen;
		const _sidebarWidth = shellLayout.sidebarWidth;
		const _expanded = files.expandedFolders;
		const _sort = files.sortOrder;
		const _terminalOpen = terminals.open;
		const _terminalHeight = terminals.height;

		scheduleWorkspaceSave();
	});
</script>

{#if vault.isUnlocked}
	<div class="flex h-screen flex-col">
		<div class="flex flex-1 overflow-hidden">
			<Sidebar
				onfileselect={handleFileSelect}
				onrenameentry={(from, to, isDir) => handleRename(from, to, isDir)}
				ondeleteentry={(path, isDir) => handleDelete(path, isDir)}
				panelOpen={shellLayout.sidebarOpen}
				bind:panelWidth={shellLayout.sidebarWidth}
			/>

			<!-- svelte-ignore a11y_no_static_element_interactions -->
			<div
				class="flex min-w-0 flex-1 overflow-hidden {dividerResizing
					? 'cursor-col-resize select-none'
					: ''}"
				bind:this={panesContainerEl}
			>
				{#each panes.list as pane, paneIndex (pane.id)}
					{#if paneIndex > 0}
						<div class="relative z-1 shrink-0 grow-0 basis-px bg-border">
							<!-- svelte-ignore a11y_no_static_element_interactions -->
							<div
								class="absolute inset-y-0 right-[-2.5px] left-[-2.5px] cursor-col-resize transition-colors duration-120 ease-out {dividerResizing
									? 'bg-brand/50'
									: 'hover:bg-brand/25'}"
								onmousedown={(e) =>
									startDividerDrag(
										e,
										paneIndex - 1,
										panesContainerEl!,
										(v) => (dividerResizing = v)
									)}
							></div>
						</div>
					{/if}
					<!-- svelte-ignore a11y_no_static_element_interactions -->
					<div
						class="relative flex min-w-0 flex-col overflow-hidden bg-background"
						style="flex: {panes.flexes[paneIndex]}"
						onmousedown={() => panes.focusPane(paneIndex)}
					>
						<PaneView
							{pane}
							{paneIndex}
							focused={paneIndex === panes.activePaneIndex}
							onrename={handleRename}
							onwikilink={handleWikiLink}
							ontabcontextmenu={handleTabContextMenu}
							attachmentFolder={vaultBootstrap.attachmentFolder}
							{dropTarget}
							ondropenter={(pi, zone) => {
								dropTarget = { paneIndex: pi, zone };
							}}
							ondropleave={(pi, zone) => {
								if (dropTarget?.paneIndex === pi && dropTarget.zone === zone) dropTarget = null;
							}}
						/>
					</div>
				{/each}
			</div>

			{#if showBacklinks && panes.activeTab && panes.activeTab.type === 'markdown'}
				<BacklinksPanel
					filePath={panes.activeTab.path}
					onclose={() => (showBacklinks = false)}
					onfileselect={handleFileSelect}
				/>
			{/if}

			{#if showHistory && panes.activeTab && panes.activeTab.type === 'markdown'}
				<HistoryPanel
					filePath={panes.activeTab.path}
					onclose={() => (showHistory = false)}
					onrestore={() => {
						editor.markLocalChange();
					}}
				/>
			{/if}
		</div>

		<TerminalPanel />

		<StatusBar
			onlogout={onLogout}
			onsettings={() => (showSettings = true)}
			onsync={runManualSync}
			onswitchvault={onLogout}
			onsidebartoggle={() => (shellLayout.sidebarOpen = !shellLayout.sidebarOpen)}
			sidebarOpen={shellLayout.sidebarOpen}
			onhistory={() => (showHistory = !showHistory)}
			historyActive={showHistory}
			ontrash={() => (showTrash = true)}
			onbacklinks={() => (showBacklinks = !showBacklinks)}
			backlinksActive={showBacklinks}
			viewMode={panes.activeTab?.type === 'markdown' ? panes.activeTab.viewMode : 'rich'}
			ontoggleviewmode={toggleViewMode}
			onterminal={() => terminals.toggle()}
			terminalActive={terminals.open}
		/>
	</div>

	{#if showTrash}
		<TrashDialog
			onclose={() => (showTrash = false)}
			onrestored={async (path) => {
				if (!vault.vaultPath) return;
				await files.revealFile(path, vault.vaultPath);
			}}
			onopen={handleFileSelect}
		/>
	{/if}

	{#if showSettings}
		<Settings onclose={() => (showSettings = false)} />
	{/if}

	{#if showSpotlight}
		<Spotlight
			onselect={handleFileSelect}
			onclose={() => (showSpotlight = false)}
			onsettings={() => (showSettings = true)}
		/>
	{/if}
{:else}
	<Login />
{/if}

{#if tabContextMenu}
	<TabContextMenu
		x={tabContextMenu.x}
		y={tabContextMenu.y}
		paneIndex={tabContextMenu.paneIndex}
		tabIndex={tabContextMenu.tabIndex}
		onclose={() => {
			tabContextMenu = null;
		}}
	/>
{/if}

<Toaster />

<svelte:window
	onkeydown={(e) => handleGlobalKeydown(e, shortcutActions)}
	onmousemove={(e) => {
		if (drag.active) drag.move(e.clientX, e.clientY);
	}}
	onmouseup={async () => {
		if (drag.active) {
			if (drag.stripTarget) {
				await executeStripDrop(drag.stripTarget);
			} else if (dropTarget) {
				await executeDrop(dropTarget);
			} else if (drag.item?.kind === 'file' && !drag.item.isDir) {
				drag.requestInsertAtCoords(drag.item.path, drag.x, drag.y);
			}
			drag.end();
			dropTarget = null;
		}
	}}
/>

<DragPreview />
