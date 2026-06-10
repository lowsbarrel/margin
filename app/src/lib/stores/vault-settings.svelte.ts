import { vault } from "$lib/stores/vault.svelte";
import { loadSettings, saveSettings } from "$lib/settings/bridge";
import type { AppSettings } from "$lib/settings/bridge";

/** Reactive settings loader — auto-loads when vault is unlocked. */
export function useVaultSettings() {
  let settings = $state<AppSettings | null>(null);

  $effect(() => {
    if (vault.vaultPath && vault.encryptionKey) {
      let cancelled = false;
      loadSettings(vault.vaultPath, vault.encryptionKey).then((s) => {
        // Ignore a stale resolution if the vault changed (or the effect re-ran)
        // before this load finished, so an older load can't clobber newer data.
        if (!cancelled) settings = s ?? null;
      });
      return () => {
        cancelled = true;
      };
    } else {
      settings = null;
    }
  });

  async function save(updated: AppSettings): Promise<boolean> {
    if (!vault.vaultPath || !vault.encryptionKey) return false;
    await saveSettings(vault.vaultPath, vault.encryptionKey, updated);
    settings = updated;
    return true;
  }

  return {
    get settings() { return settings; },
    save,
  };
}
