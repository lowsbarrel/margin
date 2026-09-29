<script lang="ts">
	import { FolderOpen } from '@lucide/svelte';
	import { FIELD, FIELD_LABEL, STEP_BODY, STEP_TITLE } from './card-styles';
	import * as m from '$lib/paraglide/messages.js';

	interface Props {
		vaultPath: string;
		onpick: () => void;
	}

	let { vaultPath, onpick }: Props = $props();
</script>

<div class="flex flex-col gap-1">
	<h2 class={STEP_TITLE}>{m.onboarding_wizard_folder_title()}</h2>
	<p class={STEP_BODY}>{m.onboarding_wizard_folder_body()}</p>
</div>
<div class="{FIELD} mt-3">
	<label for="vault-path" class={FIELD_LABEL}>
		<FolderOpen size={14} />
		{m.login_storage_label()}
	</label>
	<button
		class="w-full rounded-sm border border-border bg-surface-2 px-3.5 py-2.5 text-left text-sm text-foreground transition-colors hover:border-subtle-foreground"
		onclick={onpick}
	>
		{#if vaultPath}
			<span class="block truncate">{vaultPath}</span>
		{:else}
			<span class="text-subtle-foreground">{m.login_storage_placeholder()}</span>
		{/if}
	</button>
</div>
