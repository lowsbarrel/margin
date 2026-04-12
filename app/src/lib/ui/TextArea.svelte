<script lang="ts">
  type Size = "sm" | "md" | "lg";

  interface Props {
    value: string;
    onchange?: (value: string) => void;
    placeholder?: string;
    size?: Size;
    id?: string;
    disabled?: boolean;
    readonly?: boolean;
    mono?: boolean;
    rows?: number;
  }

  let {
    value = $bindable(),
    onchange,
    placeholder = "",
    size = "md",
    id,
    disabled = false,
    readonly = false,
    mono = true,
    rows = 3,
  }: Props = $props();

  function handleInput(e: Event) {
    const target = e.target as HTMLTextAreaElement;
    value = target.value;
    onchange?.(target.value);
  }
</script>

<textarea
  class="textarea {size}"
  class:mono
  {id}
  {placeholder}
  {disabled}
  {readonly}
  {rows}
  {value}
  oninput={handleInput}
></textarea>

<style>
  .textarea {
    width: 100%;
    background: var(--bg-tertiary);
    color: var(--text-primary);
    border: 1px solid var(--border);
    border-radius: var(--radius-sm);
    font-family: var(--font-sans);
    resize: vertical;
    transition:
      border-color 0.15s ease,
      box-shadow 0.15s ease;
  }

  .mono {
    font-family: var(--font-mono);
  }

  .textarea:focus {
    border-color: var(--text-muted);
    box-shadow: 0 0 0 3px rgba(255, 255, 255, 0.04);
    outline: none;
  }

  .textarea:disabled {
    opacity: 0.5;
    cursor: not-allowed;
  }
  .textarea::placeholder {
    color: var(--text-muted);
  }

  .sm {
    padding: 6px 10px;
    font-size: 0.75rem;
  }
  .md {
    padding: 8px 12px;
    font-size: 0.8rem;
  }
  .lg {
    padding: 10px 14px;
    font-size: 0.85rem;
  }
</style>
