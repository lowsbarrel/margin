import { isFormControlFocused, isModalOpen } from '$lib/utils/modal';

export interface ShortcutActions {
	toggleTerminal: () => void;
	toggleSpotlight: () => void;
	openSpotlight: () => void;
	reopenClosedTab: () => void;
	toggleSidebar: () => void;
	newNote: () => void;
	toggleViewMode: () => void;
}

export function handleGlobalKeydown(e: KeyboardEvent, actions: ShortcutActions): void {
	if (!(e.metaKey || e.ctrlKey)) return;
	const key = e.key.toLowerCase();
	const inControl = isFormControlFocused();

	if (!e.shiftKey && key === '`') {
		e.preventDefault();
		if (!isModalOpen()) actions.toggleTerminal();
		return;
	}

	if (!e.shiftKey && (key === 'k' || key === 'p')) {
		if (inControl) return;
		e.preventDefault();
		if (!isModalOpen()) actions.toggleSpotlight();
		return;
	}

	if (e.shiftKey && key === 'f') {
		if (inControl) return;
		e.preventDefault();
		if (!isModalOpen()) actions.openSpotlight();
		return;
	}

	if (e.shiftKey && key === 't') {
		if (inControl) return;
		e.preventDefault();
		if (!isModalOpen()) actions.reopenClosedTab();
		return;
	}

	if (!e.shiftKey && key === '\\') {
		e.preventDefault();
		if (!isModalOpen()) actions.toggleSidebar();
		return;
	}

	if (!e.shiftKey && key === 'n') {
		e.preventDefault();
		if (!isModalOpen()) actions.newNote();
	}

	if (e.shiftKey && key === 'e') {
		e.preventDefault();
		if (!isModalOpen() && !inControl) actions.toggleViewMode();
	}
}
