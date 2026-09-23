import {
	captureInsertionPoint,
	insertPastedFile,
	type AttachmentTarget
} from '$lib/editor/attachments';
import { toast } from '$lib/stores/toast.svelte';
import * as m from '$lib/paraglide/messages.js';

/**
 * Handle pasting images/files into the editor.
 * Supports clipboard DataTransfer and Windows screenshot fallback (Win+Shift+S).
 */
export function handleEditorPaste(event: ClipboardEvent, target: AttachmentTarget): void {
	const clipData = event.clipboardData;
	if (!clipData) return;

	const pastedFiles: File[] = [];
	if (clipData.items) {
		for (let i = 0; i < clipData.items.length; i++) {
			const item = clipData.items[i];
			if (item.kind === 'file') {
				const file = item.getAsFile();
				if (file) pastedFiles.push(file);
			}
		}
	}
	if (pastedFiles.length === 0 && clipData.files.length > 0) {
		for (let i = 0; i < clipData.files.length; i++) {
			pastedFiles.push(clipData.files[i]);
		}
	}

	// Fallback for Windows screenshot paste (Win+Shift+S): WebView2 sometimes
	// hides the image from the synchronous DataTransfer API but still exposes
	// it via the async Clipboard API. Detect image intent via types.
	const hasImageType =
		pastedFiles.length === 0 &&
		Array.from(clipData.types || []).some((t) => t.startsWith('image/'));

	if (pastedFiles.length === 0 && !hasImageType) return;
	event.preventDefault();
	event.stopPropagation();

	// Read before the first await: a clipboard image read is async, and the
	// caret must not be wherever the user left it by the time the bytes land.
	const at = captureInsertionPoint(target.editor);

	void (async () => {
		if (pastedFiles.length === 0 && hasImageType) {
			try {
				const items = await navigator.clipboard.read();
				for (const item of items) {
					for (const type of item.types) {
						if (!type.startsWith('image/')) continue;
						const blob = await item.getType(type);
						const ext = type.split('/')[1]?.split('+')[0] || 'png';
						pastedFiles.push(new File([blob], `pasted-${Date.now()}.${ext}`, { type }));
					}
				}
			} catch (err) {
				console.error('Clipboard image read failed:', err);
			}
		}

		let cursor = at;
		for (const file of pastedFiles) {
			try {
				cursor = await insertPastedFile(file, target, cursor);
			} catch (err) {
				toast.error(m.toast_attachment_paste_failed({ name: file.name, error: String(err) }));
			}
		}
	})();
}
