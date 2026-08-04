<script lang="ts">
	import { vault } from '$lib/stores/vault.svelte';
	import { toast } from '$lib/stores/toast.svelte';
	import { files } from '$lib/stores/files.svelte';
	import { s3Configure, s3TestConnection, type S3Config } from '$lib/s3/bridge';
	import { syncToS3, type ConflictStrategy } from '$lib/sync/s3sync';
	import { Button, Input, Field, Section } from '$lib/ui';
	import { Cloud, TestTube, Upload, RefreshCw } from '@lucide/svelte';
	import { cn } from '$lib/utils';
	import * as m from '$lib/paraglide/messages.js';

	/* Four hints and the test result share this; only the colour differs. */
	const HINT = 'm-0 font-sans text-xs italic text-subtle-foreground';

	interface Props {
		endpoint: string;
		bucket: string;
		region: string;
		accessKey: string;
		secretKey: string;
		autoSync: boolean;
		conflictStrategy: ConflictStrategy;
	}

	let {
		endpoint = $bindable(),
		bucket = $bindable(),
		region = $bindable(),
		accessKey = $bindable(),
		secretKey = $bindable(),
		autoSync = $bindable(),
		conflictStrategy = $bindable()
	}: Props = $props();

	let testing = $state(false);
	let testOk = $state<boolean | null>(null);
	let testResult = $state('');
	let syncing = $state(false);

	function getS3Config(): S3Config {
		return {
			endpoint: endpoint.trim(),
			bucket: bucket.trim(),
			region: region.trim(),
			access_key: accessKey.trim(),
			secret_key: secretKey.trim()
		};
	}

	async function handleTest() {
		testing = true;
		testOk = null;
		testResult = '';
		try {
			await s3Configure(getS3Config());
			testResult = await s3TestConnection();
			testOk = true;
		} catch (err) {
			testResult = String(err);
			testOk = false;
		} finally {
			testing = false;
		}
	}

	async function handleSync() {
		if (!vault.vaultPath || !vault.vaultId || !vault.encryptionKey) return;
		const config = getS3Config();
		if (!config.endpoint || !config.bucket) {
			toast.error(m.toast_configure_s3());
			return;
		}
		syncing = true;
		try {
			await syncToS3(vault.vaultPath, vault.vaultId, vault.encryptionKey, config, {
				conflictStrategy
			});
			if (vault.vaultPath) await files.refresh(vault.vaultPath);
			toast.success(m.toast_sync_complete());
		} catch (err) {
			toast.error(m.toast_sync_failed({ error: String(err) }));
		} finally {
			syncing = false;
		}
	}
</script>

<Section title={m.settings_s3_title()} icon={Cloud} collapsible defaultOpen={false}>
	<Field label={m.settings_endpoint()} forId="endpoint">
		<Input
			id="endpoint"
			bind:value={endpoint}
			placeholder={m.settings_endpoint_placeholder()}
			mono
		/>
	</Field>
	<Field label={m.settings_bucket()} forId="bucket">
		<Input id="bucket" bind:value={bucket} placeholder={m.settings_bucket_placeholder()} mono />
	</Field>
	<Field label={m.settings_region()} forId="region">
		<Input id="region" bind:value={region} placeholder={m.settings_region_placeholder()} mono />
	</Field>
	<Field label={m.settings_access_key()} forId="accessKey">
		<Input
			id="accessKey"
			bind:value={accessKey}
			placeholder={m.settings_access_key_placeholder()}
			mono
		/>
	</Field>
	<Field label={m.settings_secret_key()} forId="secretKey">
		<Input id="secretKey" bind:value={secretKey} type="password" placeholder="••••••••" mono />
	</Field>

	<div class="flex flex-wrap gap-2">
		<Button variant="secondary" icon={TestTube} onclick={handleTest} loading={testing}>
			{testing ? m.settings_testing() : m.settings_test()}
		</Button>
		<Button variant="success" icon={Upload} onclick={handleSync} loading={syncing}>
			{syncing ? m.settings_syncing() : m.settings_sync_now()}
		</Button>
	</div>

	{#if testResult}
		<p class={cn(HINT, testOk ? 'text-positive' : 'text-destructive')}>{testResult}</p>
	{/if}

	<div class="flex items-center justify-between py-1.5">
		<label
			class="flex cursor-pointer items-center gap-1.5 font-sans text-sm text-foreground"
			for="autoSync"
		>
			<RefreshCw size={14} />
			{m.settings_auto_sync()}
		</label>
		<!-- Hand-rolled switch: shadcn's Switch is not vendored here, and swapping
		     it in would trade this visually-hidden checkbox for a `role="switch"`
		     button — a different markup contract and a different binding. The
		     checkbox stays the source of truth and drives the track through the
		     `peer` variant.

		     The thumb slides via `transition-[translate,…]`, not `transform`:
		     Tailwind v4's `translate-x-*` compiles to the standalone `translate`
		     property, so transitioning `transform` would leave it snapping. -->
		<label class="relative inline-flex h-5 w-9 shrink-0">
			<input
				type="checkbox"
				id="autoSync"
				class="peer absolute h-0 w-0 opacity-0"
				bind:checked={autoSync}
			/>
			<span
				class="absolute inset-0 cursor-pointer rounded-full border border-border bg-surface-2 transition-colors duration-150 ease-out peer-checked:border-foreground peer-checked:bg-foreground after:absolute after:top-0.5 after:left-0.5 after:size-3.5 after:rounded-full after:bg-subtle-foreground after:transition-[translate,background-color] after:duration-150 after:ease-out after:content-[''] peer-checked:after:translate-x-4 peer-checked:after:bg-background"
			></span>
		</label>
	</div>
	{#if autoSync}
		<p class={HINT}>{m.settings_auto_sync_hint()}</p>
	{/if}

	<Field label={m.settings_conflict_resolution()} forId="conflictStrategy">
		<!-- The `@layer base` rule for `select` supplies the font, tracking and
		     the brand focus glow; these utilities restate only what the old
		     `.select-field` class overrode on top of it. -->
		<select
			class="w-full cursor-pointer rounded-sm border border-border bg-surface-2 px-3 py-2 font-sans text-sm text-foreground transition-colors duration-150 ease-out focus:border-subtle-foreground focus:outline-none"
			id="conflictStrategy"
			bind:value={conflictStrategy}
		>
			<option value="local_wins">{m.settings_conflict_local_wins()}</option>
			<option value="keep_newer">{m.settings_conflict_keep_newer()}</option>
		</select>
	</Field>
	{#if conflictStrategy === 'keep_newer'}
		<p class={HINT}>{m.settings_conflict_hint_newer()}</p>
	{:else}
		<p class={HINT}>{m.settings_conflict_hint_local()}</p>
	{/if}
</Section>
