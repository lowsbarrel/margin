<script lang="ts">
  import { Input, Field, Section } from "$lib/ui";
  import { Paperclip } from "lucide-svelte";
  import * as m from "$lib/paraglide/messages.js";

  interface Props {
    attachmentFolder: string;
    vaultFolders: string[];
  }

  let { attachmentFolder = $bindable(), vaultFolders }: Props = $props();
</script>

<Section
  title={m.settings_attachments_title()}
  icon={Paperclip}
  collapsible
  defaultOpen={false}
>
  <p class="hint">{m.settings_attachments_hint()}</p>
  <Field label={m.settings_attachments_label()} forId="attachmentFolder">
    {#if vaultFolders.length > 0}
      <select
        class="select-field"
        id="attachmentFolder"
        bind:value={attachmentFolder}
      >
        <option value="">{m.settings_attachments_none()}</option>
        {#each vaultFolders as folder}
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

<style>
  .hint {
    font-size: 0.75rem;
    color: var(--text-muted);
    margin: 0;
    font-family: var(--font-sans);
    font-style: italic;
  }

  .select-field {
    padding: 8px 12px;
    font-size: 0.8rem;
    font-family: var(--font-sans);
    background: var(--bg-tertiary);
    color: var(--text-primary);
    border: 1px solid var(--border);
    border-radius: var(--radius-sm);
    cursor: pointer;
    transition: border-color 0.15s ease;
    width: 100%;
  }

  .select-field:focus {
    border-color: var(--text-muted);
    outline: none;
  }
</style>
