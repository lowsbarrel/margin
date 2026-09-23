/**
 * Whether a modal dialog is currently mounted — Settings, Spotlight, or any
 * shadcn/bits-ui Dialog. Global shortcuts consult this so they never fire
 * behind a modal.
 */
export function isModalOpen(): boolean {
	return document.querySelector('[aria-modal="true"]') !== null;
}

/**
 * Whether focus sits in a form control, where a shortcut that moves focus (the
 * palette, a reopened tab) would swallow the keystroke, and where Space and
 * Cmd+Z belong to the control rather than to a global handler. The editor's
 * `contenteditable` is deliberately not one of these: its shortcuts stay live
 * while typing a note.
 */
export function isFormControlFocused(): boolean {
	const el = document.activeElement;
	// xterm keeps focus in a helper <textarea> so it can read keystrokes; the
	// terminal handles its own keys, so app shortcuts stay live in a shell.
	if (!el || el.closest('.xterm')) return false;
	const tag = el.tagName;
	return tag === 'INPUT' || tag === 'TEXTAREA' || tag === 'SELECT';
}
