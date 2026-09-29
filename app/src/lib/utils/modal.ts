export function isModalOpen(): boolean {
	return document.querySelector('[aria-modal="true"]') !== null;
}

export function isFormControlFocused(): boolean {
	const el = document.activeElement;
	// xterm focuses a hidden textarea to read keystrokes; its own keymap keeps app shortcuts live.
	if (!el || el.closest('.xterm')) return false;
	const tag = el.tagName;
	return tag === 'INPUT' || tag === 'TEXTAREA' || tag === 'SELECT';
}
