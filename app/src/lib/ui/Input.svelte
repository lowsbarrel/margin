<script lang="ts">
  type Size = "sm" | "md" | "lg";

  interface Props {
    value: string;
    onchange?: (value: string) => void;
    placeholder?: string;
    type?: "text" | "password" | "email" | "url";
    size?: Size;
    id?: string;
    icon?: any;
    disabled?: boolean;
    readonly?: boolean;
    mono?: boolean;
  }

  let {
    value = $bindable(),
    onchange,
    placeholder = "",
    type = "text",
    size = "md",
    id,
    icon: Icon,
    disabled = false,
    readonly = false,
    mono = false,
  }: Props = $props();

  function handleInput(e: Event) {
    const target = e.target as HTMLInputElement;
    value = target.value;
    onchange?.(target.value);
  }
</script>

<div class="input-wrap {size}" class:has-icon={!!Icon} class:mono>
  {#if Icon}
    <span class="input-icon">
      <Icon size={size === "sm" ? 12 : 14} />
    </span>
  {/if}
  <input
    {type}
    {id}
    {placeholder}
    {disabled}
    {readonly}
    {value}
    oninput={handleInput}
    class="input"
  />
</div>

<style>
  .input-wrap {
    position: relative;
    display: flex;
    align-items: center;
    width: 100%;
  }

  .input {
    width: 100%;
    background: var(--bg-tertiary);
    color: var(--text-primary);
    border: 1px solid var(--border);
    border-radius: var(--radius-sm);
    font-family: var(--font-sans);
    transition:
      border-color 0.15s ease,
      box-shadow 0.15s ease;
  }

  .mono .input {
    font-family: var(--font-mono);
  }

  .input:focus {
    border-color: var(--text-muted);
    box-shadow: 0 0 0 3px rgba(255, 255, 255, 0.04);
    outline: none;
  }

  .input:disabled {
    opacity: 0.5;
    cursor: not-allowed;
  }
  .input::placeholder {
    color: var(--text-muted);
  }

  /* Sizes */
  .sm .input {
    padding: 6px 10px;
    font-size: 0.75rem;
  }
  .md .input {
    padding: 8px 12px;
    font-size: 0.8rem;
  }
  .lg .input {
    padding: 10px 14px;
    font-size: 0.85rem;
  }

  /* Icon */
  .has-icon .input {
    padding-left: 32px;
  }
  .input-icon {
    position: absolute;
    left: 10px;
    color: var(--text-muted);
    display: flex;
    align-items: center;
    pointer-events: none;
  }
</style>
