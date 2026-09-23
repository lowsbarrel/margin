<script lang="ts">
	import { Button, Input, Field, Section } from '$lib/ui';
	import { Paperclip, Sparkles } from '@lucide/svelte';
	import { deleteEntry, unusedAttachments } from '$lib/fs/bridge';
	import { resolveAttachmentFolder } from '$lib/editor/attachments';
	import { vault } from '$lib/stores/vault.svelte';
	import { toast } from '$lib/stores/toast.svelte';
	import * as m from '$lib/paraglide/messages.js';

	interface Props {
		attachmentFolder: string;
		vaultFolders: string[];
	}

	let { attachmentFolder = $bindable(), vaultFolders }: Props = $props();

	/** The setting is optional; an empty one means the default folder. */
	const folder = $derived(resolveAttachmentFolder(attachmentFolder));

	/** Null until a scan has run — then the list, empty included. */
	let unused = $state<string[] | null>(null);
	let selected = $state<string[]>([]);
	let scanning = $state(false);
	let cleaning = $state(false);

	async function scanForUnused() {
		scanning = true;
		try {
			unused = await unusedAttachments(folder);
			selected = [...unused];
		} catch (err) {
			toast.error(m.settings_attachments_clean_failed({ error: String(err) }));
		} finally {
			scanning = false;
		}
	}

	function toggle(path: string) {
		selected = selected.includes(path) ? selected.filter((p) => p !== path) : [...selected, path];
	}

	async function moveToTrash() {
		if (!vault.vaultPath) return;
		cleaning = true;
		const removed: string[] = [];
		for (const path of selected) {
			try {
				await deleteEntry(`${vault.vaultPath}/${path}`);
				removed.push(path);
			} catch (err) {
				toast.error(m.toast_delete_failed({ error: String(err) }));
			}
		}
		// Whatever failed stays on the list instead of claiming to be gone.
		unused = (unused ?? []).filter((path) => !removed.includes(path));
		selected = [];
		cleaning = false;
		if (removed.length > 0) {
			toast.success(m.settings_attachments_clean_done({ count: String(removed.length) }));
		}
	}
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
				{#each vaultFolders as name (name)}
					<option value={name}>{name}</option>
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

	<Button variant="secondary" icon={Sparkles} fullWidth loading={scanning} onclick={scanForUnused}>
		{scanning ? m.settings_attachments_clean_scanning() : m.settings_attachments_clean_scan()}
	</Button>

	{#if unused !== null}
		{#if unused.length === 0}
			<p class="m-0 font-sans text-xs text-subtle-foreground">
				{m.settings_attachments_clean_none()}
			</p>
		{:else}
			<p class="m-0 font-sans text-xs text-subtle-foreground">
				{m.settings_attachments_clean_found({ count: String(unused.length) })}
			</p>
			<ul class="m-0 max-h-40 list-none overflow-y-auto rounded-sm border border-border p-0">
				{#each unused as path (path)}
					<li class="border-b border-border last:border-b-0">
						<label
							class="flex cursor-pointer items-center gap-2 px-2 py-1.5 font-sans text-xs text-foreground hover:bg-surface-2"
						>
							<input
								class="accent-brand"
								type="checkbox"
								checked={selected.includes(path)}
								onchange={() => toggle(path)}
							/>
							<span class="truncate" title={path}>{path}</span>
						</label>
					</li>
				{/each}
			</ul>
			<div class="flex gap-2">
				<Button
					variant="danger"
					disabled={selected.length === 0}
					loading={cleaning}
					onclick={moveToTrash}
				>
					{m.settings_attachments_clean_delete({ count: String(selected.length) })}
				</Button>
				<Button variant="ghost" onclick={() => (unused = null)}>
					{m.settings_attachments_clean_cancel()}
				</Button>
			</div>
		{/if}
	{/if}
</Section>
