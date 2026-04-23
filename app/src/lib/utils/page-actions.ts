import { panes, remapPath } from "$lib/stores/panes.svelte";
import { files } from "$lib/stores/files.svelte";
import { favourites } from "$lib/stores/favourites.svelte";
import { editor } from "$lib/stores/editor.svelte";
import { vault } from "$lib/stores/vault.svelte";
import { toast } from "$lib/stores/toast.svelte";
import {
  deleteEntry,
  unwatchFile,
  unwatchVault,
  renameEntry,
  searchFiles,
} from "$lib/fs/bridge";
import { clearHistoryTree, renameHistory } from "$lib/history/bridge";
import {
  stopAutoSync,
  clearSyncCredentials,
} from "$lib/sync/s3sync";

export async function handleRename(oldPath: string, newPath: string, isDir = false) {
  if (!vault.vaultPath || oldPath === newPath) return;
  try {
    await unwatchFile();

    let historyRenamed = false;
    try {
      await renameHistory(vault.vaultPath, oldPath, newPath);
      historyRenamed = true;
    } catch (err) {
      console.warn("Failed to rename history:", err);
    }

    try {
      await renameEntry(oldPath, newPath);
    } catch (err) {
      if (historyRenamed) {
        renameHistory(vault.vaultPath, newPath, oldPath).catch((revertErr) =>
          console.warn("Failed to revert history rename:", revertErr),
        );
      }
      throw err;
    }

    panes.remapPaths(oldPath, newPath, isDir);

    if (files.activeFile) {
      files.setActiveFile(
        remapPath(files.activeFile, oldPath, newPath, isDir),
      );
    }
    if (files.selectedFolder) {
      files.setSelectedFolder(
        remapPath(files.selectedFolder, oldPath, newPath, isDir),
      );
    }
    favourites.renamePath(oldPath, newPath);
    await files.refresh(vault.vaultPath);
    await panes.restoreWatchingForPane(panes.activePaneIndex);
  } catch (err) {
    console.error("Rename failed:", err);
    throw err;
  }
}

export async function handleDelete(path: string, isDir: boolean) {
  if (!vault.vaultPath) return;
  try {
    await unwatchFile();

    panes.removePaths(path, isDir);

    if (files.activeFile && (files.activeFile === path || (isDir && files.activeFile.startsWith(`${path}/`)))) {
      files.setActiveFile(null);
    }
    if (files.selectedFolder && (files.selectedFolder === path || (isDir && files.selectedFolder.startsWith(`${path}/`)))) {
      files.setSelectedFolder(vault.vaultPath);
    }

    try {
      await clearHistoryTree(vault.vaultPath, path);
    } catch (err) {
      console.warn("Failed to clear history for deleted entry:", err);
    }
    await deleteEntry(path);
    favourites.removePath(path);
    await files.refresh(vault.vaultPath);
    await panes.restoreWatchingForPane(panes.activePaneIndex);
  } catch (err) {
    console.error("Delete failed:", err);
    throw err;
  }
}

export async function handleWikiLink(title: string) {
  if (!vault.vaultPath) return;
  const results = await searchFiles(vault.vaultPath, title);
  const match = results.find(
    (r) =>
      !r.is_dir && (r.name === `${title}.md` || r.name === `${title}.canvas`),
  );
  if (match) {
    await panes.openFile(match.path);
  } else {
    toast.info(`Note not found: ${title}`);
  }
}

export function handleLogout(onBeforeLogout?: () => void) {
  onBeforeLogout?.();
  stopAutoSync();
  clearSyncCredentials();
  panes.reset();
  files.clear();
  unwatchFile();
  unwatchVault();
  editor.setSyncStatus("idle");
  editor.setDirty(false);
  vault.lock();
}
