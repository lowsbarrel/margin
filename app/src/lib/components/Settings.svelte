<script lang="ts">
	import { vault } from '$lib/stores/vault.svelte';
	import { toast } from '$lib/stores/toast.svelte';
	import { s3Configure, type S3Config } from '$lib/s3/bridge';
	import { saveSettings, loadSettings, type AppSettings } from '$lib/settings/bridge';
	import { startAutoSync, stopAutoSync, type ConflictStrategy } from '$lib/sync/s3sync';
	import { GlassModal } from '$lib/ui';
	import * as m from '$lib/paraglide/messages.js';
	import { llmConfigure, type ApiFormat, type Effort } from '$lib/ai/bridge';
	import { ask } from '$lib/stores/ask.svelte';
	import SettingsVault from './settings/SettingsVault.svelte';
	import SettingsCloud from './settings/SettingsCloud.svelte';
	import SettingsAi from './settings/SettingsAi.svelte';
	import SettingsLocale from './settings/SettingsLocale.svelte';
	import SettingsAppearance from './settings/SettingsAppearance.svelte';
	import SettingsExportZip from './settings/SettingsExportZip.svelte';
	import SettingsExportImport from './settings/SettingsExportImport.svelte';
	import SettingsUpdates from './settings/SettingsUpdates.svelte';

	interface Props {
		onclose: () => void;
	}

	let { onclose }: Props = $props();

	let endpoint = $state('');
	let bucket = $state('');
	let region = $state('us-east-1');
	let accessKey = $state('');
	let secretKey = $state('');
	/* No longer editable: a folder chosen before attachments became automatic is
	   carried through saves so its existing embeds keep resolving. */
	let attachmentFolder = $state('');
	let autoSync = $state(false);
	let conflictStrategy = $state<ConflictStrategy>('local_wins');
	let llmFormat = $state<ApiFormat>('openai');
	let llmBaseUrl = $state('');
	let llmApiKey = $state('');
	let llmModel = $state('');
	let llmEffort = $state<Effort | null>(null);

	$effect(() => {
		if (vault.vaultPath && vault.encryptionKey) {
			loadSettings(vault.vaultPath, vault.encryptionKey).then((settings) => {
				if (settings?.s3) {
					endpoint = settings.s3.endpoint;
					bucket = settings.s3.bucket;
					region = settings.s3.region;
					accessKey = settings.s3.access_key;
					secretKey = settings.s3.secret_key;
				}
				attachmentFolder = settings?.attachment_folder ?? '';
				autoSync = settings?.auto_sync ?? false;
				conflictStrategy = (settings?.conflict_strategy as ConflictStrategy) ?? 'local_wins';
				if (settings?.llm) {
					llmFormat = settings.llm.api_format;
					llmBaseUrl = settings.llm.base_url ?? '';
					llmApiKey = settings.llm.api_key ?? '';
					llmModel = settings.llm.model;
					llmEffort = settings.llm.effort ?? null;
				}
				// Hand the endpoint to Rust here, where the key is in hand, so the
				// answer loop never needs it over IPC again.
				if (settings?.llm) {
					llmConfigure(settings.llm)
						.then(() => ask.markConfigured(true))
						.catch((err) => {
							console.warn('Failed to configure AI:', err);
							ask.markConfigured(false);
						});
				} else {
					ask.markConfigured(false);
				}
			});
		}
	});

	function getS3Config(): S3Config {
		return {
			endpoint: endpoint.trim(),
			bucket: bucket.trim(),
			region: region.trim(),
			access_key: accessKey.trim(),
			secret_key: secretKey.trim()
		};
	}

	function getAppSettings(): AppSettings {
		const config = getS3Config();
		const hasS3 = config.endpoint && config.bucket && config.access_key && config.secret_key;
		// A model is what makes the endpoint usable; without one there is nothing
		// to ask, so the AI section stays unconfigured rather than half-saved.
		const model = llmModel.trim();
		return {
			s3: hasS3 ? config : null,
			attachment_folder: attachmentFolder.trim() || null,
			auto_sync: autoSync || null,
			conflict_strategy: conflictStrategy,
			llm: model
				? {
						api_format: llmFormat,
						base_url: llmBaseUrl.trim(),
						api_key: llmApiKey.trim(),
						model,
						effort: llmEffort
					}
				: null
		};
	}

	function handleImported(settings: AppSettings) {
		if (settings.s3) {
			endpoint = settings.s3.endpoint;
			bucket = settings.s3.bucket;
			region = settings.s3.region;
			accessKey = settings.s3.access_key;
			secretKey = settings.s3.secret_key;
		}
		attachmentFolder = settings.attachment_folder ?? '';
		autoSync = settings.auto_sync ?? false;
		conflictStrategy = (settings.conflict_strategy as ConflictStrategy) ?? 'local_wins';
		llmFormat = settings.llm?.api_format ?? 'openai';
		llmBaseUrl = settings.llm?.base_url ?? '';
		llmApiKey = settings.llm?.api_key ?? '';
		llmModel = settings.llm?.model ?? '';
		llmEffort = settings.llm?.effort ?? null;
	}

	async function handleClose() {
		await handleSave();
		onclose();
	}

	async function handleSave() {
		if (!vault.vaultPath || !vault.encryptionKey) return;
		try {
			const settings = getAppSettings();
			if (settings.s3) await s3Configure(settings.s3);
			// Rust state, not the settings file, is what the answer loop reads.
			if (settings.llm) await llmConfigure(settings.llm);
			ask.markConfigured(Boolean(settings.llm));
			await saveSettings(vault.vaultPath, vault.encryptionKey, settings);

			if (autoSync && settings.s3 && vault.vaultId && vault.encryptionKey) {
				startAutoSync(vault.vaultPath, vault.vaultId, vault.encryptionKey, settings.s3, undefined, {
					conflictStrategy
				});
			} else {
				stopAutoSync();
			}

			toast.success(m.toast_settings_saved());
		} catch (err) {
			toast.error(m.toast_save_failed({ error: String(err) }));
		}
	}
</script>

<GlassModal title={m.settings_title()} onclose={handleClose}>
	<SettingsVault />
	<SettingsCloud
		bind:endpoint
		bind:bucket
		bind:region
		bind:accessKey
		bind:secretKey
		bind:autoSync
		bind:conflictStrategy
	/>
	<SettingsAi
		bind:apiFormat={llmFormat}
		bind:baseUrl={llmBaseUrl}
		bind:apiKey={llmApiKey}
		bind:model={llmModel}
		bind:effort={llmEffort}
	/>
	<SettingsLocale />
	<SettingsAppearance />
	<SettingsExportZip />
	<SettingsExportImport getSettings={getAppSettings} onimported={handleImported} />
	<SettingsUpdates />
</GlassModal>
