export function useInlineEdit(options: {
	onSubmit: (value: string) => void;
	onCancel: () => void;
}) {
	function commit(input: HTMLInputElement) {
		const value = input.value.trim();
		if (value) options.onSubmit(value);
		else options.onCancel();
	}

	function handleKeydown(e: KeyboardEvent & { currentTarget: HTMLInputElement }) {
		if (e.key === 'Enter') {
			e.preventDefault();
			commit(e.currentTarget);
		} else if (e.key === 'Escape') {
			e.preventDefault();
			options.onCancel();
		}
	}

	// A focused row removed from the DOM still fires blur; committing there saves a name never sent.
	function handleBlur(e: FocusEvent & { currentTarget: HTMLInputElement }) {
		if (!e.currentTarget.isConnected) return;
		commit(e.currentTarget);
	}

	return { handleKeydown, handleBlur };
}

// The `autofocus` attribute only fires while the body has focus, so a click on a toolbar button left the field unfocused.
export function focusOnMount(input: HTMLInputElement): void {
	queueMicrotask(() => input.focus());
}
