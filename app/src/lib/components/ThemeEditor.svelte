<script lang="ts">
  import {
    theme,
    THEME_COLOR_KEYS,
    THEME_COLOR_LABELS,
    DARK_DEFAULTS,
    LIGHT_DEFAULTS,
    type ThemeColorKey,
  } from "$lib/stores/theme.svelte";
  import { toast } from "$lib/stores/toast.svelte";
  import { exportTheme, importTheme, type Theme } from "$lib/themes/bridge";
  import { Section, Button, Input, Field } from "$lib/ui";
  import * as m from "$lib/paraglide/messages.js";
  import {
    Palette,
    Plus,
    Trash2,
    Download,
    Upload,
    Check,
  } from "lucide-svelte";
  import {
    save as saveDialog,
    open as openDialog,
  } from "@tauri-apps/plugin-dialog";

  let editingTheme = $state<Theme | null>(null);
  let editingName = $state("");
  let editingColors = $state<Record<string, string>>({});
  let isNewTheme = $state(false);

  function defaults(): Record<ThemeColorKey, string> {
    return theme.current === "light"
      ? { ...LIGHT_DEFAULTS }
      : { ...DARK_DEFAULTS };
  }

  function startNew() {
    isNewTheme = true;
    editingName = "My Theme";
    editingColors = defaults();
    editingTheme = { name: editingName, colors: editingColors };
  }

  function startEdit(t: Theme) {
    isNewTheme = false;
    editingName = t.name;
    // Merge with defaults so all keys are present
    editingColors = { ...defaults(), ...t.colors };
    editingTheme = t;
  }

  function cancelEdit() {
    editingTheme = null;
    editingName = "";
    editingColors = {};
    theme.cancelPreview();
  }

  async function saveEdit() {
    const name = editingName.trim();
    if (!name) {
      toast.error(m.theme_name_empty());
      return;
    }
    const t: Theme = { name, colors: { ...editingColors } };
    try {
      await theme.addTheme(t);
      await theme.activateTheme(name);
      editingTheme = null;
      editingName = "";
      editingColors = {};
      toast.success(m.theme_saved({ name }));
    } catch (err) {
      toast.error(String(err));
    }
  }

  async function deleteTheme(name: string) {
    try {
      await theme.deleteTheme(name);
      if (editingTheme?.name === name) {
        editingTheme = null;
        editingName = "";
        editingColors = {};
      }
      toast.success(m.theme_deleted({ name }));
    } catch (err) {
      toast.error(String(err));
    }
  }

  async function handleExport(t: Theme) {
    const dest = await saveDialog({
      title: m.theme_export_dialog(),
      defaultPath: `${t.name.replace(/\s+/g, "-").toLowerCase()}.json`,
      filters: [{ name: "JSON", extensions: ["json"] }],
    });
    if (!dest) return;
    try {
      await exportTheme(t, dest);
      toast.success(m.theme_exported());
    } catch (err) {
      toast.error(String(err));
    }
  }

  async function handleImport() {
    const path = await openDialog({
      title: m.theme_import_dialog(),
      filters: [{ name: "JSON", extensions: ["json"] }],
      multiple: false,
    });
    if (!path) return;
    try {
      const t = await importTheme(path as string);
      await theme.addTheme(t);
      toast.success(m.theme_imported({ name: t.name }));
    } catch (err) {
      toast.error(String(err));
    }
  }

  function handleColorChange(key: string, value: string) {
    editingColors[key] = value;
    editingColors = { ...editingColors };
    theme.previewColors(editingColors);
  }
</script>

<Section title={m.theme_title()} icon={Palette} collapsible defaultOpen={false}>
  <!-- Active theme selector -->
  <div class="theme-active-row">
    <label class="theme-label" for="activeTheme">{m.theme_active()}</label>
    <select
      class="select-field"
      id="activeTheme"
      value={theme.activeThemeName ?? ""}
      onchange={(e) => {
        const val = (e.target as HTMLSelectElement).value;
        theme.activateTheme(val || null);
      }}
    >
      <option value="">{m.theme_default({ mode: theme.current })}</option>
      {#each theme.customThemes as t}
        <option value={t.name}>{t.name}</option>
      {/each}
    </select>
  </div>

  <!-- Saved themes list -->
  {#if theme.customThemes.length > 0}
    <div class="theme-list">
      {#each theme.customThemes as t}
        <div class="theme-item" class:active={theme.activeThemeName === t.name}>
          <div class="theme-swatches">
            {#each ["bg-primary", "bg-secondary", "accent", "accent-link"] as key}
              <span
                class="swatch"
                style="background: {t.colors[key] ??
                  defaults()[key as ThemeColorKey]}"
              ></span>
            {/each}
          </div>
          <span class="theme-name">{t.name}</span>
          <div class="theme-actions">
            <button class="icon-btn" title={m.theme_edit()} onclick={() => startEdit(t)}>
              <Palette size={12} />
            </button>
            <button
              class="icon-btn"
              title={m.theme_export()}
              onclick={() => handleExport(t)}
            >
              <Download size={12} />
            </button>
            <button
              class="icon-btn danger"
              title={m.theme_delete()}
              onclick={() => deleteTheme(t.name)}
            >
              <Trash2 size={12} />
            </button>
          </div>
        </div>
      {/each}
    </div>
  {/if}

  <!-- Action buttons -->
  <div class="theme-btns">
    <Button variant="secondary" icon={Plus} onclick={startNew} size="sm"
      >{m.theme_new()}</Button
    >
    <Button variant="secondary" icon={Upload} onclick={handleImport} size="sm"
      >{m.theme_import_json()}</Button
    >
  </div>

  <!-- Editor -->
  {#if editingTheme}
    <div class="theme-editor">
      <Field label={m.theme_name_label()} forId="themeName">
        <Input id="themeName" bind:value={editingName} placeholder={m.theme_name_placeholder()} />
      </Field>

      <div class="color-grid">
        {#each THEME_COLOR_KEYS as key}
          <div class="color-row">
            <label class="color-label" for="color-{key}"
              >{THEME_COLOR_LABELS[key]}</label
            >
            <div class="color-input-group">
              <input
                type="color"
                id="color-{key}"
                value={editingColors[key] ?? defaults()[key]}
                oninput={(e) =>
                  handleColorChange(key, (e.target as HTMLInputElement).value)}
              />
              <input
                class="color-hex"
                type="text"
                value={editingColors[key] ?? defaults()[key]}
                onchange={(e) =>
                  handleColorChange(key, (e.target as HTMLInputElement).value)}
              />
            </div>
          </div>
        {/each}
      </div>

      <div class="editor-actions">
        <Button variant="secondary" onclick={cancelEdit} size="sm"
          >{m.theme_cancel()}</Button
        >
        <Button variant="primary" icon={Check} onclick={saveEdit} size="sm">
          {isNewTheme ? m.theme_create() : m.theme_save_btn()}
        </Button>
      </div>
    </div>
  {/if}
</Section>

<style>
  .theme-active-row {
    display: flex;
    align-items: center;
    justify-content: space-between;
    gap: var(--space-sm);
  }

  .theme-label {
    font-size: 0.8rem;
    color: var(--text-primary);
    white-space: nowrap;
  }

  .select-field {
    padding: 6px 10px;
    font-size: 0.8rem;
    font-family: var(--font-sans);
    background: var(--bg-tertiary);
    color: var(--text-primary);
    border: 1px solid var(--border);
    border-radius: var(--radius-sm);
    cursor: pointer;
    flex: 1;
    max-width: 200px;
  }

  .select-field:focus {
    border-color: var(--text-muted);
    outline: none;
  }

  .theme-list {
    display: flex;
    flex-direction: column;
    gap: 4px;
    margin-top: var(--space-sm);
  }

  .theme-item {
    display: flex;
    align-items: center;
    gap: var(--space-sm);
    padding: 6px 8px;
    border-radius: var(--radius-xs);
    background: var(--bg-secondary);
    border: 1px solid var(--border-subtle);
    transition: background 0.1s;
  }

  .theme-item.active {
    border-color: var(--accent);
  }

  .theme-swatches {
    display: flex;
    gap: 3px;
    flex-shrink: 0;
  }

  .swatch {
    width: 14px;
    height: 14px;
    border-radius: 3px;
    border: 1px solid var(--border);
  }

  .theme-name {
    flex: 1;
    font-size: 0.8rem;
    color: var(--text-primary);
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }

  .theme-actions {
    display: flex;
    gap: 2px;
    flex-shrink: 0;
  }

  .icon-btn {
    display: grid;
    place-items: center;
    width: 24px;
    height: 24px;
    padding: 0;
    background: none;
    border: none;
    color: var(--text-muted);
    border-radius: var(--radius-xs);
    cursor: pointer;
    transition: all 0.1s;
  }

  .icon-btn:hover {
    background: var(--bg-hover);
    color: var(--text-primary);
  }

  .icon-btn.danger:hover {
    color: var(--danger);
  }

  .theme-btns {
    display: flex;
    gap: var(--space-sm);
    margin-top: var(--space-sm);
  }

  .theme-editor {
    margin-top: var(--space-md);
    padding: var(--space-md);
    background: var(--bg-secondary);
    border: 1px solid var(--border);
    border-radius: var(--radius-sm);
    display: flex;
    flex-direction: column;
    gap: var(--space-md);
  }

  .color-grid {
    display: flex;
    flex-direction: column;
    gap: 6px;
  }

  .color-row {
    display: flex;
    align-items: center;
    justify-content: space-between;
    gap: var(--space-sm);
  }

  .color-label {
    font-size: 0.75rem;
    color: var(--text-secondary);
    min-width: 100px;
  }

  .color-input-group {
    display: flex;
    align-items: center;
    gap: 6px;
  }

  input[type="color"] {
    width: 28px;
    height: 28px;
    padding: 0;
    border: 1px solid var(--border);
    border-radius: var(--radius-xs);
    cursor: pointer;
    background: none;
    -webkit-appearance: none;
    appearance: none;
  }

  input[type="color"]::-webkit-color-swatch-wrapper {
    padding: 2px;
  }

  input[type="color"]::-webkit-color-swatch {
    border: none;
    border-radius: 2px;
  }

  .color-hex {
    width: 80px;
    padding: 4px 6px;
    font-family: var(--font-mono);
    font-size: 0.7rem;
    background: var(--bg-tertiary);
    color: var(--text-primary);
    border: 1px solid var(--border);
    border-radius: var(--radius-xs);
  }

  .color-hex:focus {
    border-color: var(--text-muted);
    outline: none;
  }

  .editor-actions {
    display: flex;
    justify-content: flex-end;
    gap: var(--space-sm);
  }
</style>
