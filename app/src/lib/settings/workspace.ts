import { commands } from "$lib/bindings";

export type {
  WorkspaceTab,
  WorkspacePane,
  WorkspaceState,
} from "$lib/bindings";
import type { WorkspaceState } from "$lib/bindings";

export async function saveWorkspaceState(
  vaultPath: string,
  encryptionKey: number[],
  state: WorkspaceState,
): Promise<void> {
  const r = await commands.saveWorkspaceState(vaultPath, encryptionKey, state);
  if (r.status === "error") throw r.error;
}

export async function loadWorkspaceState(
  vaultPath: string,
  encryptionKey: number[],
): Promise<WorkspaceState | null> {
  const r = await commands.loadWorkspaceState(vaultPath, encryptionKey);
  if (r.status === "error") throw r.error;
  return r.data;
}
