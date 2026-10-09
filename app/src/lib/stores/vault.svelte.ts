import {
	clearSession,
	saveSession,
	loadVaultProfiles,
	saveVaultProfile,
	type VaultProfiles
} from '$lib/session/bridge';

interface VaultState {
	isUnlocked: boolean;
	vaultId: string | null;
	vaultPath: string | null;
	profileName: string | null;
}

const state = $state<VaultState>({
	isUnlocked: false,
	vaultId: null,
	vaultPath: null,
	profileName: null
});

export const vault = {
	get isUnlocked() {
		return state.isUnlocked;
	},
	get vaultId() {
		return state.vaultId;
	},
	get vaultPath() {
		return state.vaultPath;
	},

	// mnemonicToSave is the value the user just typed; it is forwarded to Rust, never kept here.
	unlock(vaultId: string, vaultPath: string, profileName?: string, mnemonicToSave?: string) {
		const normalised = vaultPath.replaceAll('\\', '/');
		state.isUnlocked = true;
		state.vaultId = vaultId;
		state.vaultPath = normalised;
		state.profileName = profileName ?? null;
		if (mnemonicToSave) {
			if (profileName) {
				saveVaultProfile(profileName, normalised, mnemonicToSave).catch((err) =>
					console.warn('Failed to save vault profile:', err)
				);
			} else {
				saveSession(mnemonicToSave, normalised).catch((err) =>
					console.warn('Failed to save session:', err)
				);
			}
		}
	},

	lock() {
		state.isUnlocked = false;
		state.vaultId = null;
		state.vaultPath = null;
		state.profileName = null;
		// Also clears the Rust key and vault path.
		clearSession().catch((err) => console.warn('Failed to clear session:', err));
	},

	get profileName() {
		return state.profileName;
	},

	set profileName(name: string | null) {
		state.profileName = name;
	},

	async getVaultProfiles(): Promise<VaultProfiles> {
		try {
			return await loadVaultProfiles();
		} catch (err) {
			console.warn('Failed to load vault profiles:', err);
			return { profiles: [], last_used: null };
		}
	}
};
