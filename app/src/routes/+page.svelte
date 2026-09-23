<script lang="ts">
	import { onMount, onDestroy, untrack } from 'svelte';
	import { vault } from '$lib/stores/vault.svelte';
	import { files } from '$lib/stores/files.svelte';
	import { editor } from '$lib/stores/editor.svelte';
	import Login from '$lib/components/Login.svelte';
	import Sidebar from '$lib/components/Sidebar.svelte';
	import PaneView from '$lib/components/PaneView.svelte';
	import StatusBar from '$lib/components/StatusBar.svelte';
	import Settings from '$lib/components/Settings.svelte';
	import HistoryPanel from '$lib/components/HistoryPanel.svelte';
	import TrashDialog from '$lib/components/TrashDialog.svelte';
	import BacklinksPanel from '$lib/components/BacklinksPanel.svelte';
	import TerminalPanel from '$lib/components/TerminalPanel.svelte';
	import { terminals } from '$lib/stores/terminals.svelte';
	import {
		readFileBytes,
		onFileChanged,
		watchVault,
		unwatchFile,
		onVaultFsChanged,
		hasUnsyncedChanges,
		rebuildIndex
	} from '$lib/fs/bridge';
	import { loadSettings } from '$lib/settings/bridge';
	import { saveSnapshot } from '$lib/history/bridge';
	import { s3Configure } from '$lib/s3/bridge';
	import { flushEditorWrites, isOwnRecentWrite } from '$lib/fs/writeQueue';
	import { getCurrentWindow } from '@tauri-apps/api/window';
	import {
		startAutoSync,
		stopAutoSync,
		runManualSync,
		setSyncCredentials,
		type ConflictStrategy
	} from '$lib/sync/s3sync';
	import { Toaster } from '$lib/ui';
	import * as m from '$lib/paraglide/messages.js';
	import { drag } from '$lib/stores/drag.svelte';
	import ContextMenu from '$lib/components/ContextMenu.svelte';
	import type { ContextMenuItem } from '$lib/components/ContextMenu.svelte';
	import Spotlight from '$lib/components/Spotlight.svelte';
	import { checkForAppUpdate } from '$lib/utils/updater';
	import {
		saveWorkspaceState,
		loadWorkspaceState,
		type WorkspaceState
	} from '$lib/settings/workspace';
	import { panes } from '$lib/stores/panes.svelte';
	import {
		handleRename,
		handleDelete,
		handleNewNote,
		handleWikiLink,
		handleLogout
	} from '$lib/utils/page-actions';
	import { isModalOpen } from '$lib/utils/modal';
	import { executeDrop, startDividerDrag } from '$lib/utils/tab-drag';
	import { DEFAULT_ATTACHMENT_FOLDER, resolveAttachmentFolder } from '$lib/editor/attachments';

	// Local UI state

	let showSettings = $state(false);
	let showSpotlight = $state(false);
	let showHistory = $state(false);
	let showTrash = $state(false);
	let showBacklinks = $state(false);
	let sidebarOpen = $state(true);
	let sidebarWidth = $state(280);
	/** Where attachments land — the setting, or its default. */
	let attachmentFolder = $state<string>(DEFAULT_ATTACHMENT_FOLDER);
	let pendingScrollText = $state<string | null>(null);
	let unlistenFileChange: (() => void) | null = null;
	let unlistenVaultChange: (() => void) | null = null;
	let unlistenCloseRequested: (() => void) | null = null;
	let vaultRefreshTimer: ReturnType<typeof setTimeout> | null = null;
	let destroyed = false;
	let closing = false;
	let updateInterval: ReturnType<typeof setInterval> | null = null;
	let panesContainerEl = $state<HTMLElement | null>(null);
	let dividerResizing = $state(false);
	let dropTarget = $state<{
		paneIndex: number;
		zone: 'left' | 'center' | 'right';
	} | null>(null);

	// Scroll helpers

	function scrollEditorToText(searchText: string) {
		const tiptap = editor.tiptap;
		if (!tiptap?.state?.doc) {
			pendingScrollText = searchText;
			return;
		}
		doScrollToText(tiptap, searchText);
	}

	function doScrollToText(tiptap: import('@tiptap/core').Editor, searchText: string) {
		const doc = tiptap.state.doc;
		let targetPos = -1;
		doc.descendants((node, pos) => {
			if (targetPos >= 0) return false;
			if (node.isText && node.text?.includes(searchText)) {
				targetPos = pos + node.text.indexOf(searchText);
				return false;
			}
		});
		if (targetPos < 0) return;
		tiptap.commands.setTextSelection(targetPos);
		try {
			const view = tiptap.view;
			const coords = view.coordsAtPos(targetPos);
			const scrollContainer = view.dom.closest('.editor-container');
			if (scrollContainer && coords) {
				const rect = scrollContainer.getBoundingClientRect();
				const relativeTop = coords.top - rect.top + scrollContainer.scrollTop;
				scrollContainer.scrollTo({
					top: relativeTop - rect.height / 2,
					behavior: 'smooth'
				});
			}
		} catch {
			/* ignore scroll errors */
		}
	}

	// File select

	async function handleFileSelect(path: string, searchText?: string) {
		const opened = await panes.openFile(path);
		if (opened && searchText) scrollEditorToText(searchText);
	}

	// Editor surface (rich / raw Markdown). `activeTab` hands back the reactive
	// tab object, so writing the field is enough to move the surface and to have
	// the workspace autosave effect notice.
	function toggleViewMode() {
		const tab = panes.activeTab;
		if (!tab || tab.type !== 'markdown') return;
		tab.viewMode = tab.viewMode === 'source' ? 'rich' : 'source';
	}

	// Tab context menu

	let tabContextMenu = $state<{
		x: number;
		y: number;
		paneIndex: number;
		tabIndex: number;
	} | null>(null);

	function handleTabContextMenu(e: MouseEvent, paneIndex: number, tabIndex: number) {
		e.preventDefault();
		e.stopPropagation();
		tabContextMenu = { x: e.clientX, y: e.clientY, paneIndex, tabIndex };
	}

	function getTabContextMenuItems(paneIndex: number, tabIndex: number): ContextMenuItem[] {
		const pane = panes.list[paneIndex];
		const tab = pane.tabs[tabIndex];
		return [
			{
				label: tab?.pinned ? m.tab_unpin() : m.tab_pin(),
				onclick: () => panes.togglePin(paneIndex, tabIndex)
			},
			{ label: m.tab_close(), onclick: () => panes.closeTab(paneIndex, tabIndex) },
			{
				label: m.tab_close_others(),
				onclick: () => panes.closeOtherTabs(paneIndex, tabIndex),
				disabled: pane.tabs.length <= 1
			},
			{ label: m.tab_close_all(), onclick: () => panes.closeAllTabs(paneIndex) },
			{
				label: m.tab_reopen_closed(),
				onclick: () => {
					panes.reopenClosedTab();
				},
				disabled: !panes.canReopenClosedTab
			}
		];
	}

	// Logout / switch vault

	function onLogout() {
		handleLogout(() => {
			doSaveWorkspaceState();
			if (workspaceSaveTimer) clearTimeout(workspaceSaveTimer);
			workspaceRestored = false;
		});
	}

	// Workspace state persistence

	let workspaceSaveTimer: ReturnType<typeof setTimeout> | null = null;
	let workspaceRestored = false;

	function scheduleWorkspaceSave() {
		if (!vault.isUnlocked || !vault.vaultPath || !vault.encryptionKey) return;
		if (!workspaceRestored) return;
		if (workspaceSaveTimer) clearTimeout(workspaceSaveTimer);
		workspaceSaveTimer = setTimeout(() => {
			workspaceSaveTimer = null;
			doSaveWorkspaceState();
		}, 1000);
	}

	function doSaveWorkspaceState() {
		if (!vault.vaultPath || !vault.encryptionKey) return;
		const wsState: WorkspaceState = {
			panes: panes.list.map((p) => ({
				tabs: p.tabs.map((t) => ({
					path: t.path,
					type: t.type,
					pinned: t.pinned,
					view_mode: t.viewMode,
					cursor_pos: t.cursorPos ?? null
				})),
				active_tab_index: p.activeTabIndex
			})),
			pane_flexes: [...panes.flexes],
			active_pane_index: panes.activePaneIndex,
			expanded_folders: [...files.expandedFolders],
			sidebar_open: sidebarOpen,
			sidebar_width: sidebarWidth,
			sort_order: files.sortOrder,
			terminal_open: terminals.open,
			terminal_height: terminals.height
		};
		saveWorkspaceState(vault.vaultPath, vault.encryptionKey, wsState).catch((err) =>
			console.warn('Failed to save workspace state:', err)
		);
	}

	async function restoreWorkspaceState() {
		if (!vault.vaultPath || !vault.encryptionKey) return;
		try {
			const ws = await loadWorkspaceState(vault.vaultPath, vault.encryptionKey);
			if (!ws || ws.panes.length === 0) {
				workspaceRestored = true;
				return;
			}

			sidebarOpen = ws.sidebar_open;
			sidebarWidth = ws.sidebar_width ?? sidebarWidth;
			terminals.height = ws.terminal_height ?? terminals.height;
			// Tabs are not persisted — a shell is a process, not a file — so an
			// open panel restores as one fresh terminal.
			if (ws.terminal_open) terminals.toggle();

			if (
				(ws.sort_order === 'name' || ws.sort_order === 'date') &&
				ws.sort_order !== files.sortOrder
			) {
				files.setSortOrder(ws.sort_order);
			}

			for (const folder of ws.expanded_folders) {
				files.expandedFolders.add(folder);
			}
			await files.refresh(vault.vaultPath);

			await panes.restoreFromWorkspace(
				ws.panes,
				ws.pane_flexes.map((f) => f ?? 1),
				ws.active_pane_index
			);
		} catch (err) {
			console.warn('Failed to restore workspace state:', err);
		}
		workspaceRestored = true;
	}

	// Effects

	$effect(() => {
		const tiptap = editor.tiptap;
		const text = pendingScrollText;
		if (tiptap?.state?.doc && text) {
			pendingScrollText = null;
			doScrollToText(tiptap, text);
		}
	});

	// Lifecycle

	onMount(() => {
		checkForAppUpdate();
		updateInterval = setInterval(checkForAppUpdate, 5 * 60 * 1000);

		onFileChanged(async () => {
			const tab = panes.activeTab;
			if (!tab) return;
			try {
				const bytes = await readFileBytes(tab.path);
				// Our own saves echo back through the watcher. Recognise them by
				// content — a time window either swallows a real external edit that
				// lands inside it or lets our own save through after it.
				if (isOwnRecentWrite(tab.path, bytes)) return;
				const newContent = new TextDecoder().decode(bytes);
				if (newContent === tab.content) return;
				// This is a genuine external change and applying it replaces whatever
				// the editor holds. Snapshot the version being replaced first so an
				// external edit can never cost the user their local text.
				if (vault.vaultPath) {
					try {
						await saveSnapshot(vault.vaultPath, tab.path, new TextEncoder().encode(tab.content));
					} catch (err) {
						console.warn('Failed to snapshot before applying external change:', err);
					}
				}
				if (panes.applyExternalContent(tab.path, newContent)) {
					editor.markLocalChange();
				}
			} catch (err) {
				console.warn('File may have been deleted:', err);
			}
		}).then((unlisten) => {
			// If the component unmounted before this promise resolved, the onDestroy
			// ref was still null — detach immediately so the listener doesn't leak.
			if (destroyed) unlisten();
			else unlistenFileChange = unlisten;
		});

		onVaultFsChanged(() => {
			editor.markLocalChange();
			if (vaultRefreshTimer) clearTimeout(vaultRefreshTimer);
			vaultRefreshTimer = setTimeout(() => {
				vaultRefreshTimer = null;
				// Best-effort: a failed refresh only leaves the tree stale until the next fs event.
				files.refresh().catch((err) => console.warn('Failed to refresh file tree:', err));
				// Keep the full-text search index fresh after any vault change (in-app
				// save, sync, git, external edit). Skips unchanged files, so this only
				// re-reads what actually changed. Fire-and-forget; best-effort.
				if (vault.vaultPath) void rebuildIndex(vault.vaultPath).catch(() => {});
			}, 300);
		}).then((unlisten) => {
			if (destroyed) unlisten();
			else unlistenVaultChange = unlisten;
		});

		// Flush pending debounced editor saves and the write queue before the
		// window closes so no in-flight edit is lost on quit.
		getCurrentWindow()
			.onCloseRequested(async (event) => {
				// Re-entrancy guard: ignore further close clicks while we're already
				// tearing down (otherwise each click stacks another preventDefault).
				if (closing) return;
				closing = true;
				event.preventDefault();
				try {
					// Stop autosync now so it can't keep enqueuing writes while we drain;
					// otherwise the flush loop may never see settled.size hit 0.
					stopAutoSync();
					// Shells are separate processes: closing the window must take them
					// with it, not leave them running in the vault.
					terminals.reset();
					// Bound the flush: a stuck/looping write queue (locked file, stalled
					// IPC) must never be able to wedge the window permanently open.
					await Promise.race([
						flushEditorWrites(),
						new Promise((resolve) => setTimeout(resolve, 3000))
					]);
				} finally {
					// Always close, even if the flush threw or timed out — losing a few
					// ms of unsaved edits beats an app you can't quit.
					try {
						await getCurrentWindow().destroy();
					} catch (err) {
						// destroy() can reject (e.g. missing capability). Don't stay wedged:
						// clear the guard so another X click can retry instead of no-op'ing.
						console.error('[close] window.destroy() failed:', err);
						closing = false;
					}
				}
			})
			.then((unlisten) => {
				if (destroyed) unlisten();
				else unlistenCloseRequested = unlisten;
			});
	});

	onDestroy(() => {
		destroyed = true;
		if (workspaceSaveTimer) clearTimeout(workspaceSaveTimer);
		if (vaultRefreshTimer) clearTimeout(vaultRefreshTimer);
		doSaveWorkspaceState();
		if (updateInterval) clearInterval(updateInterval);
		stopAutoSync();
		unwatchFile();
		unlistenFileChange?.();
		unlistenVaultChange?.();
		unlistenCloseRequested?.();
	});

	// Vault setup

	// Narrow a persisted/IPC string to a known ConflictStrategy instead of an
	// unchecked cast, so a corrupted settings file falls back to the default.
	function parseConflictStrategy(value: unknown): ConflictStrategy {
		return value === 'keep_newer' ? 'keep_newer' : 'local_wins';
	}

	$effect(() => {
		if (vault.isUnlocked && vault.vaultPath) {
			const currentVaultPath = vault.vaultPath;
			const currentKey = vault.encryptionKey;
			untrack(() => {
				files
					.refresh(currentVaultPath)
					.catch((err) => console.warn('Failed to load file tree:', err));
				restoreWorkspaceState();
				watchVault(currentVaultPath).catch((err) =>
					console.warn('Failed to start vault watcher:', err)
				);
				if (currentKey) {
					loadSettings(currentVaultPath, currentKey)
						.then((settings) => {
							if (vault.vaultPath !== currentVaultPath) return;
							if (settings?.s3) {
								// s3sync re-configures before each sync, where a failure surfaces with a toast.
								s3Configure(settings.s3).catch((err) =>
									console.warn('Failed to configure S3:', err)
								);
								const syncOpts = {
									conflictStrategy: parseConflictStrategy(settings?.conflict_strategy)
								};
								if (vault.vaultId && vault.encryptionKey) {
									setSyncCredentials(
										currentVaultPath,
										vault.vaultId,
										vault.encryptionKey,
										settings.s3,
										syncOpts
									);
								}
								if (currentKey) {
									hasUnsyncedChanges(currentVaultPath, currentKey)
										.then((pending) => {
											if (vault.vaultPath !== currentVaultPath) return;
											if (editor.syncStatus === 'syncing') return;
											editor.setSyncStatus(pending ? 'idle' : 'synced');
										})
										.catch(() => {
											if (editor.syncStatus === 'syncing') return;
											editor.setSyncStatus('idle');
										});
								}
								if (settings.auto_sync && vault.vaultId && vault.encryptionKey) {
									startAutoSync(
										currentVaultPath,
										vault.vaultId,
										vault.encryptionKey,
										settings.s3,
										undefined,
										syncOpts
									);
								}
							} else {
								editor.setSyncStatus('idle');
							}
							attachmentFolder = resolveAttachmentFolder(settings?.attachment_folder);
						})
						.catch((err) => {
							console.warn('Failed to load settings:', err);
						});
				}
			});
		}
	});

	// The attachments folder is hidden from the file tree, and only from the
	// tree: sync, export, the watcher and filename search all still see it.
	$effect(() => {
		if (!vault.vaultPath) return;
		files.setHiddenPaths([`${vault.vaultPath}/${attachmentFolder}`]);
	});

	// Auto-save workspace state when layout changes
	$effect(() => {
		const _panes = panes.list.map((p) => ({
			tabs: p.tabs.map((t) => [t.path, t.viewMode]),
			activeTabIndex: p.activeTabIndex
		}));
		const _flexes = panes.flexes;
		const _activePane = panes.activePaneIndex;
		const _sidebarOpen = sidebarOpen;
		const _sidebarWidth = sidebarWidth;
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
				panelOpen={sidebarOpen}
				ontoggle={() => (sidebarOpen = !sidebarOpen)}
				bind:panelWidth={sidebarWidth}
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
						<!-- The divider stays a true hairline; the grab affordance is an orange
						     ring on hover (and for the whole drag) rather than a thicker bar
						     that would shift the layout. -->
						<div
							class="z-1 shrink-0 grow-0 basis-px cursor-col-resize bg-border transition-shadow duration-120 ease-out hover:ring-1 hover:ring-brand {dividerResizing
								? 'ring-1 ring-brand'
								: ''}"
							onmousedown={(e) =>
								startDividerDrag(e, paneIndex - 1, panesContainerEl!, (v) => (dividerResizing = v))}
						></div>
					{/if}
					<!-- svelte-ignore a11y_no_static_element_interactions -->
					<div
						class="relative flex min-w-0 flex-col overflow-hidden bg-background"
						class:pane-active={paneIndex === panes.activePaneIndex}
						style="flex: {panes.flexes[paneIndex]}"
						onmousedown={() => panes.focusPane(paneIndex)}
					>
						<PaneView
							{pane}
							{paneIndex}
							onrename={handleRename}
							onwikilink={handleWikiLink}
							ontabcontextmenu={handleTabContextMenu}
							{attachmentFolder}
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
			onsidebartoggle={() => (sidebarOpen = !sidebarOpen)}
			{sidebarOpen}
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
				await files.refresh();
				await handleFileSelect(path);
			}}
		/>
	{/if}

	{#if showSettings}
		<Settings
			onclose={() => (showSettings = false)}
			onattachmentschange={(folder) => (attachmentFolder = folder)}
		/>
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
	<ContextMenu
		x={tabContextMenu.x}
		y={tabContextMenu.y}
		items={getTabContextMenuItems(tabContextMenu.paneIndex, tabContextMenu.tabIndex)}
		onclose={() => {
			tabContextMenu = null;
		}}
	/>
{/if}

<Toaster />

<svelte:window
	onkeydown={(e) => {
		if (!(e.metaKey || e.ctrlKey)) return;
		// Shift changes the character the key reports ("f" → "F"), so compare on a
		// lowercased key and test the modifier separately.
		const key = e.key.toLowerCase();

		// Terminal panel. Matched on the character rather than the physical key:
		// layouts where `\` sits on that key (the plan gives Ctrl+\ to the sidebar)
		// keep their sidebar shortcut.
		if (!e.shiftKey && key === '`') {
			e.preventDefault();
			terminals.toggle();
			return;
		}

		// Spotlight. Cmd/Ctrl+K is the primary binding; Cmd/Ctrl+P is kept as an
		// alias for the quick switcher it grew out of, and Cmd/Ctrl+Shift+F — which
		// used to focus the (now removed) sidebar search — opens the same palette.
		if (!e.shiftKey && (key === 'k' || key === 'p')) {
			e.preventDefault();
			showSpotlight = !showSpotlight;
			return;
		}
		if (e.shiftKey && key === 'f') {
			e.preventDefault();
			showSpotlight = true;
			return;
		}
		if (e.shiftKey && key === 't') {
			e.preventDefault();
			panes.reopenClosedTab();
			return;
		}
		// Neither of these may fire behind a modal — a note created under a
		// Settings dialog would be invisible until the dialog closed.
		if (!e.shiftKey && key === '\\') {
			e.preventDefault();
			if (!isModalOpen()) sidebarOpen = !sidebarOpen;
			return;
		}
		if (!e.shiftKey && key === 'n') {
			e.preventDefault();
			if (!isModalOpen()) handleNewNote();
		}
		// Toggle the active tab's editor surface.
		if (e.shiftKey && key === 'e') {
			e.preventDefault();
			toggleViewMode();
		}
	}}
	onmousemove={(e) => {
		if (drag.active) drag.move(e.clientX, e.clientY);
	}}
	onmouseup={async () => {
		if (drag.active) {
			if (dropTarget) {
				await executeDrop(dropTarget);
			} else if (drag.item?.kind === 'file' && !drag.item.isDir) {
				// No split zone hit — signal the editor under the cursor to insert at that position
				drag.requestInsertAtCoords(drag.item.path, drag.x, drag.y);
			}
			drag.end();
			dropTarget = null;
		}
	}}
/>

{#if drag.active}
	<div
		class="pointer-events-none fixed z-9999 rounded-sm border border-border bg-background px-2.5 py-1.25 text-xs font-medium whitespace-nowrap text-foreground shadow-(--shadow-lg)"
		style="left: {drag.x + 14}px; top: {drag.y - 12}px"
	>
		{drag.item?.label}
	</div>
{/if}
