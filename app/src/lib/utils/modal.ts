/**
 * Whether a modal dialog is currently mounted — Settings, Spotlight, or any
 * shadcn/bits-ui Dialog. Global shortcuts consult this so they never fire
 * behind a modal.
 */
export function isModalOpen(): boolean {
	return document.querySelector('[aria-modal="true"]') !== null;
}
