/**
 * Lifecycle for a one-line inline editor: Enter submits, Escape cancels, a real
 * blur submits. Every inline input in the tree (rename, new folder) is built on
 * this, so the rules cannot drift apart between them.
 */
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

	// A row unmounted by virtualization while its input is focused reports the
	// detachment as a blur. Committing there would apply a half-typed name the
	// user never submitted, so an input no longer in the document is ignored.
	function handleBlur(e: FocusEvent & { currentTarget: HTMLInputElement }) {
		if (!e.currentTarget.isConnected) return;
		commit(e.currentTarget);
	}

	return { handleKeydown, handleBlur };
}
