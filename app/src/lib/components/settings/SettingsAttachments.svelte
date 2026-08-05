<script lang="ts">
	import { Input, Field, Section } from '$lib/ui';
	import { Paperclip } from '@lucide/svelte';
	import * as m from '$lib/paraglide/messages.js';

	interface Props {
		attachmentFolder: string;
		vaultFolders: string[];
	}

	let { attachmentFolder = $bindable(), vaultFolders }: Props = $props();
</script>

<Section title={m.settings_attachments_title()} icon={Paperclip} collapsible defaultOpen={false}>
	<p class="m-0 font-sans text-xs text-subtle-foreground italic">
		{m.settings_attachments_hint()}
	</p>
	<Field label={m.settings_attachments_label()} forId="attachmentFolder">
		{#if vaultFolders.length > 0}
			<!-- The `@layer base` rule for `select` supplies the font, tracking and
			     the brand focus glow; these utilities restate only what the old
			     `.select-field` class overrode on top of it. -->
			<select
				class="w-full cursor-pointer rounded-sm border border-border bg-surface-2 px-3 py-2 font-sans text-sm text-foreground transition-colors duration-150 ease-out focus:border-subtle-foreground focus:outline-none"
				id="attachmentFolder"
				bind:value={attachmentFolder}
			>
				<option value="">{m.settings_attachments_none()}</option>
				{#each vaultFolders as folder (folder)}
					<option value={folder}>{folder}</option>
				{/each}
			</select>
		{:else}
			<Input
				id="attachmentFolder"
				bind:value={attachmentFolder}
				placeholder={m.settings_attachments_folder_placeholder()}
			/>
		{/if}
	</Field>
</Section>
