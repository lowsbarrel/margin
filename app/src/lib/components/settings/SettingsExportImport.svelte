<script lang="ts">
	import { vault } from '$lib/stores/vault.svelte';
	import { toast } from '$lib/stores/toast.svelte';
	import { s3Configure } from '$lib/s3/bridge';
	import {
		saveSettings,
		exportSettingsString,
		importSettingsString,
		type AppSettings
	} from '$lib/settings/bridge';
	import { startAutoSync, stopAutoSync, type ConflictStrategy } from '$lib/sync/s3sync';
	import { Button, TextArea, Section } from '$lib/ui';
	import { Upload, Download, Copy, Check } from '@lucide/svelte';
	import * as m from '$lib/paraglide/messages.js';

	interface Props {
		getSettings: () => AppSettings;
		onimported: (settings: AppSettings) => void;
	}

	let { getSettings, onimported }: Props = $props();

	let exportString = $state('');
	let importString = $state('');
	let copied = $state(false);

	async function handleExport() {
		if (!vault.encryptionKey) return;
		try {
			exportString = await exportSettingsString(vault.encryptionKey, getSettings());
		} catch (err) {
			toast.error(m.toast_export_failed({ error: String(err) }));
		}
	}

	async function handleCopy() {
		await navigator.clipboard.writeText(exportString);
		copied = true;
		toast.success(m.toast_copied());
		setTimeout(() => (copied = false), 2000);
	}

	async function handleImport() {
		if (!vault.encryptionKey || !vault.vaultPath || !importString.trim()) return;
		try {
			const settings = await importSettingsString(vault.encryptionKey, importString.trim());
			if (settings.s3) await s3Configure(settings.s3);
			await saveSettings(vault.vaultPath, vault.encryptionKey, settings);

			if (settings.auto_sync && settings.s3 && vault.vaultId && vault.encryptionKey) {
				startAutoSync(vault.vaultPath, vault.vaultId, vault.encryptionKey, settings.s3, undefined, {
					conflictStrategy: (settings.conflict_strategy as ConflictStrategy) ?? 'local_wins'
				});
			} else {
				stopAutoSync();
			}

			onimported(settings);
			importString = '';
			toast.success(m.toast_import_success());
		} catch (err) {
			toast.error(m.toast_import_failed({ error: String(err) }));
		}
	}
</script>

<Section title={m.settings_export_title()} icon={Upload} collapsible defaultOpen={false}>
	<p class="m-0 font-sans text-xs text-subtle-foreground italic">{m.settings_export_hint()}</p>
	<Button variant="secondary" onclick={handleExport} fullWidth>
		{m.settings_export_generate()}
	</Button>
	{#if exportString}
		<div class="relative">
			<TextArea value={exportString} readonly rows={3} />
			<!-- Floats over the scrollable textarea, so it blurs whatever text
			     passes beneath it rather than sitting on an opaque plate. -->
			<button
				class="absolute top-2 right-2 flex size-[26px] cursor-pointer items-center justify-center rounded-xs border border-border bg-background p-0 text-subtle-foreground backdrop-blur-md transition-colors duration-150 ease-out hover:text-foreground"
				onclick={handleCopy}
			>
				{#if copied}<Check size={14} />{:else}<Copy size={14} />{/if}
			</button>
		</div>
	{/if}
</Section>

<Section title={m.settings_import_title()} icon={Download} collapsible defaultOpen={false}>
	<p class="m-0 font-sans text-xs text-subtle-foreground italic">{m.settings_import_hint()}</p>
	<TextArea bind:value={importString} placeholder={m.settings_import_placeholder()} rows={3} />
	<Button
		variant="secondary"
		icon={Download}
		onclick={handleImport}
		disabled={!importString.trim()}
		fullWidth
	>
		{m.settings_import_btn()}
	</Button>
</Section>
