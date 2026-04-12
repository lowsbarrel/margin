import type { VaultKeys } from "$lib/crypto/bridge";
import {
  saveSession,
  loadSession,
  clearSession,
  loadVaultProfiles,
  saveVaultProfile,
  type VaultProfile,
  type VaultProfiles,
} from "$lib/session/bridge";

interface VaultState {
  isUnlocked: boolean;
  vaultId: string | null;
  encryptionKey: number[] | null;
  vaultPath: string | null;
  mnemonic: string | null;
  profileName: string | null;
}

let state = $state<VaultState>({
  isUnlocked: false,
  vaultId: null,
  encryptionKey: null,
  vaultPath: null,
  mnemonic: null,
  profileName: null,
});

export const vault = {
  get isUnlocked() {
    return state.isUnlocked;
  },
  get vaultId() {
    return state.vaultId;
  },
  get encryptionKey() {
    return state.encryptionKey;
  },
  get vaultPath() {
    return state.vaultPath;
  },

  unlock(
    keys: VaultKeys,
    vaultPath: string,
    mnemonic?: string,
    profileName?: string,
  ) {
    state.isUnlocked = true;
    state.vaultId = keys.vault_id;
    state.encryptionKey = keys.encryption_key;
    state.vaultPath = vaultPath;
    state.profileName = profileName ?? null;
    if (mnemonic) {
      state.mnemonic = mnemonic;
      if (profileName) {
        saveVaultProfile({
          name: profileName,
          mnemonic,
          vault_path: vaultPath,
        }).catch((err) => console.warn("Failed to save vault profile:", err));
      } else {
        saveSession(mnemonic, vaultPath).catch((err) =>
          console.warn("Failed to save session:", err),
        );
      }
    }
  },

  lock() {
    state.isUnlocked = false;
    state.vaultId = null;
    state.encryptionKey = null;
    state.vaultPath = null;
    state.mnemonic = null;
    state.profileName = null;
    clearSession().catch((err) =>
      console.warn("Failed to clear session:", err),
    );
  },

  setMnemonic(m: string) {
    state.mnemonic = m;
  },

  get mnemonic() {
    return state.mnemonic;
  },

  get profileName() {
    return state.profileName;
  },

  set profileName(name: string | null) {
    state.profileName = name;
  },

  async getSavedSession(): Promise<VaultProfile | null> {
    try {
      return await loadSession();
    } catch (err) {
      console.warn("Failed to load saved session:", err);
      return null;
    }
  },

  async getVaultProfiles(): Promise<VaultProfiles> {
    try {
      return await loadVaultProfiles();
    } catch (err) {
      console.warn("Failed to load vault profiles:", err);
      return { profiles: [], last_used: null };
    }
  },
};
