<script lang="ts">
	import { vault } from '$lib/stores/vault.svelte';
	import { toast } from '$lib/stores/toast.svelte';
	import { saveVaultProfile } from '$lib/session/bridge';
	import { Button, Input, Field, Section } from '$lib/ui';
	import { KeyRound, Eye, EyeOff, FolderOpen } from '@lucide/svelte';
	import * as m from '$lib/paraglide/messages.js';

	let showPassphrase = $state(false);
	// Seeded from the store and re-seeded whenever the active vault's name
	// changes, but still writable so the <Input> can bind to it while editing
	// (a writable $derived, not $state mirrored by an effect).
	let editingVaultName = $derived(vault.profileName ?? '');

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
					<span class="font-mono text-sm tracking-widest text-subtle-foreground"
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
