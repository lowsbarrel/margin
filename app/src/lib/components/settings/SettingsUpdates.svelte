<script lang="ts">
	import { Button, Section } from '$lib/ui';
	import { ArrowDownToLine, RefreshCw } from '@lucide/svelte';
	import { checkForAppUpdate } from '$lib/utils/updater';
	import * as m from '$lib/paraglide/messages.js';

	let checkingUpdate = $state(false);
	let updateResult = $state<string | null>(null);

	async function handleCheckUpdate() {
		if (checkingUpdate) return;
		checkingUpdate = true;
		updateResult = null;
		try {
			const { check } = await import('@tauri-apps/plugin-updater');
			const update = await check();
			if (update) {
				updateResult = m.settings_updates_found({ version: update.version });
				checkForAppUpdate();
			} else {
				updateResult = m.settings_updates_none();
			}
		} catch {
			updateResult = m.settings_updates_none();
		} finally {
			checkingUpdate = false;
		}
	}
</script>

<Section title={m.settings_updates_title()} icon={ArrowDownToLine} collapsible defaultOpen={false}>
	<Button
		variant="secondary"
		icon={RefreshCw}
		onclick={handleCheckUpdate}
		loading={checkingUpdate}
		fullWidth
	>
		{checkingUpdate ? m.settings_updates_checking() : m.settings_updates_check()}
	</Button>
	{#if updateResult}
		<p class="mt-2 mb-0 font-sans text-xs text-subtle-foreground italic">{updateResult}</p>
	{/if}
</Section>
