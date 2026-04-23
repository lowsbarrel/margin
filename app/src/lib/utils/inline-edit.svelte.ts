/** Inline edit: Enter to submit, Escape to cancel, blur to submit. */
export function useInlineEdit(options: {
  onSubmit: (value: string) => void;
  onCancel: () => void;
}) {
  function handleKeydown(e: KeyboardEvent & { currentTarget: HTMLInputElement }) {
    if (e.key === "Enter") {
      e.preventDefault();
      const value = e.currentTarget.value.trim();
      if (value) options.onSubmit(value);
      else options.onCancel();
    } else if (e.key === "Escape") {
      options.onCancel();
    }
  }

  function handleBlur(e: FocusEvent & { currentTarget: HTMLInputElement }) {
    const value = e.currentTarget.value.trim();
    if (value) options.onSubmit(value);
    else options.onCancel();
  }

  return { handleKeydown, handleBlur };
}
