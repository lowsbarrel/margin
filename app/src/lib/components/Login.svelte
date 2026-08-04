<script lang="ts">
	import { onMount } from 'svelte';
	import { generateMnemonic, deriveVaultKeys } from '$lib/crypto/bridge';
	import {
		setVaultDirectory,
		fileExists,
		readFileBytes,
		writeFileBytes,
		createDirectory
	} from '$lib/fs/bridge';
	import { vault } from '$lib/stores/vault.svelte';
	import { theme } from '$lib/stores/theme.svelte';
	import { toast } from '$lib/stores/toast.svelte';
	import { deleteVaultProfile, type VaultProfile } from '$lib/session/bridge';
	import { open } from '@tauri-apps/plugin-dialog';
	import { Button, IconButton } from '$lib/ui';
	import {
		Sun,
		Moon,
		FolderOpen,
		KeyRound,
		Plus,
		Copy,
		Check,
		Trash2,
		ArrowLeft
	} from '@lucide/svelte';
	import * as m from '$lib/paraglide/messages.js';

	let mnemonic = $state('');
	let vaultPath = $state('');
	let vaultName = $state('');
	let error = $state('');
	let generatedMnemonic = $state('');
	let showGenerated = $state(false);
	let loading = $state(false);
	let copied = $state(false);
	let autoLogging = $state(false);

	let profiles = $state<VaultProfile[]>([]);
	let showNewVault = $state(false);

	// ── Class recipes ────────────────────────────────────────────────────────
	// The unlock, create and import screens are three variations on the same
	// card, so the repeated chrome is named once here rather than copied across
	// the three branches below. CARD deliberately carries no `gap-*` — the vault
	// list and the form space their children differently, and two gap utilities
	// on one element resolve by Tailwind's sort order, not by class order.
	const CARD =
		'flex w-full flex-col rounded-md border border-border bg-background p-6 shadow-[var(--shadow-lg)]';
	const BRAND = 'mb-8 text-center';
	const BRAND_TITLE = 'font-sans text-2xl font-bold tracking-[0.08em] text-foreground';
	const FIELD = 'flex flex-col gap-1.5';
	const FIELD_LABEL = 'flex items-center gap-1.5 text-xs font-medium text-subtle-foreground';
	// Shared shell for the text input and the passphrase textarea; only the face
	// differs (sans vs mono), so that stays at the call site.
	const CONTROL =
		'w-full rounded-sm border border-border bg-surface-2 px-3.5 py-2.5 text-sm leading-normal text-foreground caret-foreground transition-colors placeholder:text-subtle-foreground focus:border-ring focus:outline-none';
	// Text-only buttons: back, generate, add-vault.
	const QUIET_BTN =
		'flex items-center gap-1.5 bg-transparent font-sans text-sm text-muted-foreground transition-colors hover:text-foreground';
	const DIVIDER = 'my-1 border-t border-border';
	const ERROR_TEXT = 'font-sans text-sm text-destructive italic';

	onMount(async () => {
		const data = await vault.getVaultProfiles();
		profiles = data.profiles;

		if (data.last_used) {
			const lastProfile = data.profiles.find((p) => p.vault_path === data.last_used);
			if (lastProfile) {
				autoLogging = true;
				autoLogin(lastProfile);
			}
		}

		if (profiles.length === 0) {
			showNewVault = true;
		}
	});

	async function autoLogin(profile: VaultProfile) {
		try {
			const keys = await deriveVaultKeys(profile.mnemonic);
			await verifyOrInitVaultId(profile.vault_path, keys.vault_id);
			await setVaultDirectory(profile.vault_path);
			vault.unlock(keys, profile.vault_path, profile.mnemonic, profile.name);
		} catch (err) {
			console.warn('Auto-login failed:', err);
			vault.lock();
			autoLogging = false;
		}
	}

	async function selectProfile(profile: VaultProfile) {
		loading = true;
		error = '';
		try {
			const keys = await deriveVaultKeys(profile.mnemonic);
			await verifyOrInitVaultId(profile.vault_path, keys.vault_id);
			await setVaultDirectory(profile.vault_path);
			vault.unlock(keys, profile.vault_path, profile.mnemonic, profile.name);
		} catch (e) {
			error = String(e);
		} finally {
			loading = false;
		}
	}

	async function removeProfile(profile: VaultProfile) {
		try {
			await deleteVaultProfile(profile.vault_path);
			profiles = profiles.filter((p) => p.vault_path !== profile.vault_path);
			toast.success(m.login_vault_removed());
			if (profiles.length === 0) {
				showNewVault = true;
			}
		} catch (e) {
			toast.error(String(e));
		}
	}

	async function handleGenerate() {
		try {
			generatedMnemonic = await generateMnemonic();
			showGenerated = true;
			error = '';
		} catch (e) {
			error = String(e);
		}
	}

	/** Write vault_id to .margin/vault.id on first open; verify on subsequent. */
	async function verifyOrInitVaultId(vaultPath: string, vaultId: string): Promise<void> {
		const idFile = `${vaultPath}/.margin/vault.id`;
		if (await fileExists(idFile)) {
			const stored = new TextDecoder().decode(await readFileBytes(idFile)).trim();
			if (stored !== vaultId) {
				throw new Error(m.login_error_wrong_passphrase());
			}
		} else {
			await createDirectory(`${vaultPath}/.margin`);
			await writeFileBytes(idFile, new TextEncoder().encode(vaultId));
		}
	}

	function useGenerated() {
		mnemonic = generatedMnemonic;
		showGenerated = false;
	}

	async function copyMnemonic() {
		await navigator.clipboard.writeText(generatedMnemonic);
		copied = true;
		toast.success(m.toast_copied());
		setTimeout(() => (copied = false), 2000);
	}

	async function pickDirectory() {
		const selected = await open({ directory: true, multiple: false });
		if (selected) {
			vaultPath = (selected as string).replaceAll('\\', '/');
			if (!vaultName.trim()) {
				const parts = vaultPath.split('/');
				vaultName = parts[parts.length - 1] || '';
			}
		}
	}

	async function handleOpen() {
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
			const keys = await deriveVaultKeys(mnemonic.trim());
			await verifyOrInitVaultId(vaultPath, keys.vault_id);
			await setVaultDirectory(vaultPath);
			vault.unlock(keys, vaultPath, mnemonic.trim(), name);
		} catch (e) {
			error = String(e);
		} finally {
			loading = false;
		}
	}

	function goBack() {
		showNewVault = false;
		mnemonic = '';
		vaultPath = '';
		vaultName = '';
		error = '';
		showGenerated = false;
		generatedMnemonic = '';
	}
</script>

<div class="relative h-screen overflow-y-auto p-8">
	<!-- Short windows can't centre a full card and still show it: below 760px
	     tall the shell stops stretching and pins itself to the top instead. -->
	<div
		class="relative mx-auto flex min-h-[calc(100vh_-_4rem)] w-[min(100%,380px)] flex-col items-stretch justify-center [@media(max-height:760px)]:min-h-0 [@media(max-height:760px)]:justify-start [@media(max-height:760px)]:pt-10 [@media(max-height:760px)]:pb-4"
	>
		{#if autoLogging}
			<div class="flex flex-col items-center gap-6">
				<div class={BRAND}>
					<img src="/logo.svg" alt="Margin logo" class="mx-auto mb-3 size-20" />
					<h1 class={BRAND_TITLE}>{m.app_name()}</h1>
				</div>
				<span class="text-sm text-subtle-foreground italic">{m.login_auto_opening()}</span>
			</div>
		{:else}
			<div class="fixed top-4 right-4">
				<IconButton
					icon={theme.current === 'dark' ? Sun : Moon}
					onclick={() => theme.toggle()}
					title={m.statusbar_toggle_theme()}
				/>
			</div>

			<div class={BRAND}>
				<img src="/logo.svg" alt="Margin logo" class="mx-auto mb-3 size-20" />
				<h1 class={BRAND_TITLE}>{m.app_name()}</h1>
				<p class="mt-1.5 font-sans text-sm text-subtle-foreground italic">{m.app_tagline()}</p>
			</div>

			{#if !showNewVault && profiles.length > 0}
				<!-- Vault selector -->
				<div class="{CARD} gap-2">
					<p class="mb-1 font-sans text-sm font-medium text-subtle-foreground">
						{m.login_choose_vault()}
					</p>
					{#each profiles as profile (profile.vault_path)}
						<div
							class="flex w-full cursor-pointer items-center justify-between rounded-sm border border-border bg-surface-2 px-3.5 py-2.5 text-left transition-colors hover:border-subtle-foreground hover:bg-surface-1"
							onclick={() => selectProfile(profile)}
							role="button"
							tabindex="0"
							onkeydown={(e: KeyboardEvent) => {
								if (e.key === 'Enter') selectProfile(profile);
							}}
						>
							<div class="flex flex-col gap-0.5 overflow-hidden">
								<span class="font-sans text-sm font-medium text-foreground">{profile.name}</span>
								<span class="truncate font-mono text-xs text-subtle-foreground"
									>{profile.vault_path}</span
								>
							</div>
							<button
								class="flex size-7 shrink-0 items-center justify-center rounded-xs bg-transparent p-0 text-subtle-foreground transition-colors hover:text-destructive"
								onclick={(e: MouseEvent) => {
									e.stopPropagation();
									removeProfile(profile);
								}}
								title={m.login_remove_vault()}
							>
								<Trash2 size={14} />
							</button>
						</div>
					{/each}

					{#if error}
						<p class={ERROR_TEXT}>{error}</p>
					{/if}

					<div class={DIVIDER}></div>
					<button
						class="{QUIET_BTN} justify-center p-2"
						onclick={() => {
							showNewVault = true;
						}}
					>
						<Plus size={14} />
						{m.login_add_vault()}
					</button>
				</div>
			{:else}
				<!-- New vault form -->
				<div class="{CARD} gap-3.5">
					{#if profiles.length > 0}
						<button class="{QUIET_BTN} px-0 py-1" onclick={goBack}>
							<ArrowLeft size={14} />
							{m.login_back_to_vaults()}
						</button>
					{/if}

					<div class={FIELD}>
						<label for="vault-name" class={FIELD_LABEL}>
							{m.login_vault_name_label()}
						</label>
						<input
							id="vault-name"
							type="text"
							bind:value={vaultName}
							placeholder={m.login_vault_name_placeholder()}
							class="{CONTROL} font-sans"
						/>
					</div>

					<div class={FIELD}>
						<label for="mnemonic" class={FIELD_LABEL}>
							<KeyRound size={14} />
							{m.login_passphrase_label()}
						</label>
						<textarea
							id="mnemonic"
							bind:value={mnemonic}
							placeholder={m.login_passphrase_placeholder()}
							rows="3"
							spellcheck="false"
							class="{CONTROL} resize-none font-mono"
						></textarea>
					</div>

					<div class={FIELD}>
						<label for="vault-path" class={FIELD_LABEL}>
							<FolderOpen size={14} />
							{m.login_storage_label()}
						</label>
						<button
							class="w-full rounded-sm border border-border bg-surface-2 px-3.5 py-2.5 text-left text-sm text-foreground transition-colors hover:border-subtle-foreground"
							onclick={pickDirectory}
						>
							{#if vaultPath}
								<span class="block truncate">{vaultPath}</span>
							{:else}
								<span class="text-subtle-foreground">{m.login_storage_placeholder()}</span>
							{/if}
						</button>
					</div>

					{#if error}
						<p class={ERROR_TEXT}>{error}</p>
					{/if}

					<Button variant="primary" fullWidth onclick={handleOpen} {loading}>
						{loading ? m.login_opening() : m.login_open_vault()}
					</Button>

					<div class={DIVIDER}></div>

					{#if showGenerated}
						<div class="flex flex-col gap-3">
							<p class="font-sans text-sm text-muted-foreground italic">
								{m.login_generated_hint()}
							</p>
							<div
								class="relative rounded-sm border border-dashed border-border bg-surface-2 p-3 pr-10 font-mono text-sm leading-[1.6] text-foreground [word-spacing:0.3em]"
							>
								<p>{generatedMnemonic}</p>
								<button
									class="absolute top-2 right-2 flex bg-transparent p-1 text-subtle-foreground transition-colors hover:text-foreground"
									onclick={copyMnemonic}
									aria-label="Copy"
								>
									{#if copied}
										<Check size={14} />
									{:else}
										<Copy size={14} />
									{/if}
								</button>
							</div>
							<Button variant="primary" fullWidth onclick={useGenerated}>
								{m.login_use_passphrase()}
							</Button>
						</div>
					{:else}
						<button class="{QUIET_BTN} justify-center p-2" onclick={handleGenerate}>
							<Plus size={14} />
							{m.login_generate_new()}
						</button>
					{/if}
				</div>
			{/if}
		{/if}
	</div>
</div>
