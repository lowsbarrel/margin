<script lang="ts">
	import { vault } from '$lib/stores/vault.svelte';
	import { toast } from '$lib/stores/toast.svelte';
	import { saveVaultProfile } from '$lib/session/bridge';
	import { cleanVault } from '$lib/editor/clean-vault';
	import { Button, Input, Field, Section } from '$lib/ui';
	import { KeyRound, Eye, EyeOff, FolderOpen, Sparkles } from '@lucide/svelte';
	import * as m from '$lib/paraglide/messages.js';

	let showPassphrase = $state(false);
	// Seeded from the store and re-seeded whenever the active vault's name
	// changes, but still writable so the <Input> can bind to it while editing
	// (a writable $derived, not $state mirrored by an effect).
	let editingVaultName = $derived(vault.profileName ?? '');
	let cleaning = $state(false);
	let confirmingClean = $state(false);
	let cleanProgress = $state('');

	async function handleSaveVaultName() {
		if (!vault.vaultPath || !vault.mnemonic) return;
		const name = editingVaultName.trim() || 'Vault';
		try {
			await saveVaultProfile({
				name,
				mnemonic: vault.mnemonic,
				vault_path: vault.vaultPath
			});
			vault.profileName = name;
			toast.success(m.toast_settings_saved());
		} catch (err) {
			toast.error(String(err));
		}
	}

	async function runCleanVault() {
		if (!vault.vaultPath || cleaning) return;
		confirmingClean = false;
		cleaning = true;
		cleanProgress = m.settings_cleanup_progress({ done: '0', total: '…' });
		try {
			const res = await cleanVault(vault.vaultPath, vault.encryptionKey, (p) => {
				cleanProgress = m.settings_cleanup_progress({
					done: String(p.scanned),
					total: String(p.total)
				});
			});
			if (res.cleaned > 0) {
				toast.success(m.toast_cleanup_done({ count: String(res.cleaned) }));
			} else {
				toast.info(m.toast_cleanup_none());
			}
		} catch (err) {
			toast.error(m.toast_cleanup_failed({ error: String(err) }));
		} finally {
			cleaning = false;
			cleanProgress = '';
		}
	}
</script>

<Section title={m.settings_vault_title()} icon={KeyRound} collapsible defaultOpen={true}>
	<Field label={m.settings_vault_name()} forId="vaultName">
		<div class="flex items-center gap-2">
			<Input
				id="vaultName"
				bind:value={editingVaultName}
				placeholder={m.login_vault_name_placeholder()}
			/>
			<Button variant="secondary" onclick={handleSaveVaultName}>{m.settings_save_short()}</Button>
		</div>
	</Field>

	<Field label={m.settings_vault_passphrase()} forId="vaultPassphrase">
		<div class="flex items-center gap-2 rounded-sm border border-border bg-surface-2 px-3 py-2">
			<div class="flex-1 overflow-hidden">
				{#if showPassphrase}
					<span class="font-mono text-sm leading-[1.6] text-foreground [word-spacing:0.3em]"
						>{vault.mnemonic ?? ''}</span
					>
				{:else}
					<span class="font-mono text-sm tracking-[0.1em] text-subtle-foreground"
						>••••••••••••••••••••••••</span
					>
				{/if}
			</div>
			<button
				class="flex size-7 shrink-0 cursor-pointer items-center justify-center rounded-xs border-none bg-transparent p-0 text-subtle-foreground transition-colors duration-150 ease-out hover:text-foreground"
				onclick={() => (showPassphrase = !showPassphrase)}
				title={showPassphrase ? m.settings_vault_hide() : m.settings_vault_show()}
			>
				{#if showPassphrase}
					<EyeOff size={14} />
				{:else}
					<Eye size={14} />
				{/if}
			</button>
		</div>
	</Field>

	<Field label={m.settings_vault_location()} forId="vaultLocation">
		<div
			class="flex items-center gap-2 rounded-sm border border-border bg-surface-2 px-3 py-2 text-subtle-foreground"
		>
			<FolderOpen size={14} />
			<span class="truncate font-mono text-sm text-foreground">{vault.vaultPath ?? ''}</span>
		</div>
	</Field>
</Section>

<Section title={m.settings_cleanup_title()} icon={Sparkles} collapsible defaultOpen={false}>
	<p class="mt-0 mb-2 text-sm leading-normal text-subtle-foreground">{m.settings_cleanup_desc()}</p>
	{#if cleaning}
		<Button variant="secondary" loading disabled>{cleanProgress}</Button>
	{:else if confirmingClean}
		<div class="flex items-center gap-2">
			<Button variant="primary" onclick={runCleanVault}>{m.settings_cleanup_confirm()}</Button>
			<Button variant="ghost" onclick={() => (confirmingClean = false)}
				>{m.settings_cleanup_cancel()}</Button
			>
		</div>
	{:else}
		<Button variant="secondary" onclick={() => (confirmingClean = true)}
			>{m.settings_cleanup_button()}</Button
		>
	{/if}
</Section>
