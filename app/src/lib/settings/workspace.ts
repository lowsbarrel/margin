import { invoke } from "@tauri-apps/api/core";

export interface WorkspaceTab {
  path: string;
  type: string;
}

export interface WorkspacePane {
  tabs: WorkspaceTab[];
  active_tab_index: number;
}

export interface WorkspaceState {
  panes: WorkspacePane[];
  pane_flexes: number[];
  active_pane_index: number;
  expanded_folders: string[];
  sidebar_open: boolean;
  sidebar_width: number;
  sidebar_view: string;
  sort_order: string;
}

export async function saveWorkspaceState(
  vaultPath: string,
  encryptionKey: number[],
  state: WorkspaceState,
): Promise<void> {
  return invoke("save_workspace_state", {
    vaultPath,
    encryptionKey,
    state,
  });
}

export async function loadWorkspaceState(
  vaultPath: string,
  encryptionKey: number[],
): Promise<WorkspaceState | null> {
  return invoke<WorkspaceState | null>("load_workspace_state", {
    vaultPath,
    encryptionKey,
  });
}
