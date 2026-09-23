import { generateMnemonic } from '$lib/crypto/bridge';
import { deleteVaultProfile, type VaultProfile } from '$lib/session/bridge';
import { toast } from '$lib/stores/toast.svelte';
import { open } from '@tauri-apps/plugin-dialog';
import * as m from '$lib/paraglide/messages.js';
import { loadVaultProfiles, unlockVault } from './vault-session';

export interface LoginState {
	readonly profiles: VaultProfile[];
	readonly error: string;
	readonly loading: boolean;
	readonly autoLogging: boolean;
	mnemonic: string;
	readonly vaultPath: string;
	vaultName: string;
	readonly generatedMnemonic: string;
	readonly showGenerated: boolean;
	readonly copied: boolean;
	load(): Promise<void>;
	selectProfile(profile: VaultProfile): Promise<void>;
	removeProfile(profile: VaultProfile): Promise<boolean>;
	pickDirectory(): Promise<void>;
	generatePassphrase(): Promise<void>;
	useGeneratedPassphrase(): void;
	copyPassphrase(): Promise<void>;
	openVault(): Promise<void>;
	resetWizard(): void;
}

export function createLoginState(): LoginState {
	let profiles = $state<VaultProfile[]>([]);
	let error = $state('');
	let loading = $state(false);
	let autoLogging = $state(false);

	let mnemonic = $state('');
	let vaultPath = $state('');
	let vaultName = $state('');
	let generatedMnemonic = $state('');
	let showGenerated = $state(false);
	let copied = $state(false);

	async function load(): Promise<void> {
		const stored = await loadVaultProfiles();
		profiles = stored.profiles;
		if (!stored.lastUsed) return;
		autoLogging = true;
		autoLogin(stored.lastUsed);
	}

	async function autoLogin(profile: VaultProfile): Promise<void> {
		try {
			await unlockVault(profile.mnemonic, profile.vault_path, profile.name);
		} catch (err) {
			// `vault.lock()` clears the stored session, so a transient failure must not call it.
			console.warn('Auto-login failed:', err);
			error = String(err);
			autoLogging = false;
		}
	}

	async function selectProfile(profile: VaultProfile): Promise<void> {
		loading = true;
		error = '';
		try {
			await unlockVault(profile.mnemonic, profile.vault_path, profile.name);
		} catch (e) {
			error = String(e);
		} finally {
			loading = false;
		}
	}

	async function removeProfile(profile: VaultProfile): Promise<boolean> {
		try {
			await deleteVaultProfile(profile.vault_path);
			profiles = profiles.filter((p) => p.vault_path !== profile.vault_path);
			toast.success(m.login_vault_removed());
			return true;
		} catch (e) {
			toast.error(String(e));
			return false;
		}
	}

	async function pickDirectory(): Promise<void> {
		const selected = await open({ directory: true, multiple: false });
		if (!selected) return;
		vaultPath = (selected as string).replaceAll('\\', '/');
		if (vaultName.trim()) return;
		const parts = vaultPath.split('/');
		vaultName = parts[parts.length - 1] || '';
	}

	async function generatePassphrase(): Promise<void> {
		try {
			generatedMnemonic = await generateMnemonic();
			showGenerated = true;
			error = '';
		} catch (e) {
			error = String(e);
		}
	}

	function useGeneratedPassphrase(): void {
		mnemonic = generatedMnemonic;
		showGenerated = false;
	}

	async function copyPassphrase(): Promise<void> {
		await navigator.clipboard.writeText(generatedMnemonic);
		copied = true;
		toast.success(m.toast_copied());
		setTimeout(() => (copied = false), 2000);
	}

	async function openVault(): Promise<void> {
		if (!mnemonic.trim()) {
			error = m.login_error_no_mnemonic();
			return;
		}
		if (!vaultPath.trim()) {
			error = m.login_error_no_path();
			return;
		}

		loading = true;
		error = '';
		try {
			const name = vaultName.trim() || vaultPath.split('/').pop() || 'Vault';
			await unlockVault(mnemonic.trim(), vaultPath, name);
		} catch (e) {
			error = String(e);
		} finally {
			loading = false;
		}
	}

	function resetWizard(): void {
		mnemonic = '';
		vaultPath = '';
		vaultName = '';
		error = '';
		generatedMnemonic = '';
		showGenerated = false;
	}

	return {
		get profiles() {
			return profiles;
		},
		get error() {
			return error;
		},
		get loading() {
			return loading;
		},
		get autoLogging() {
			return autoLogging;
		},
		get mnemonic() {
			return mnemonic;
		},
		set mnemonic(value: string) {
			mnemonic = value;
		},
		get vaultPath() {
			return vaultPath;
		},
		get vaultName() {
			return vaultName;
		},
		set vaultName(value: string) {
			vaultName = value;
		},
		get generatedMnemonic() {
			return generatedMnemonic;
		},
		get showGenerated() {
			return showGenerated;
		},
		get copied() {
			return copied;
		},
		load,
		selectProfile,
		removeProfile,
		pickDirectory,
		generatePassphrase,
		useGeneratedPassphrase,
		copyPassphrase,
		openVault,
		resetWizard
	};
}
