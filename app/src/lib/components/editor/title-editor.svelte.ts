import { untrack } from 'svelte';
import { fileExists } from '$lib/fs/bridge';
import { fileTitle } from '$lib/stores/panes.svelte';
import { toast } from '$lib/stores/toast.svelte';
import { validateName } from '$lib/utils/filename';
import { baseName, parentDir } from '$lib/utils/path';
import * as m from '$lib/paraglide/messages.js';

const RENAME_DELAY = 150;

export interface TitleEditorHost {
	path(): string;
	setPath(path: string): void;
	isAlive(): boolean;
	focusEditor(): void;
	openSlashMenu(): void;
	element(): HTMLElement | undefined;
	onrename(): ((oldPath: string, newPath: string) => void | Promise<void>) | undefined;
}

export class TitleEditor {
	private readonly host: TitleEditorHost;
	text = $state('');
	pending = false;
	private timer: ReturnType<typeof setTimeout> | undefined;

	constructor(host: TitleEditorHost, initialTitle: string) {
		this.host = host;
		this.text = untrack(() => initialTitle);
	}

	input(raw: string): void {
		if (!this.host.isAlive() || !raw) return;
		if (raw === fileTitle(this.host.path())) return;

		clearTimeout(this.timer);
		this.timer = setTimeout(() => {
			if (!this.host.isAlive()) return;
			const error = validateName(raw);
			if (error) {
				toast.error(error);
				return;
			}
			const path = this.host.path();
			const dir = parentDir(path);
			const newPath = `${dir}/${raw}.md`;
			if (newPath !== path) void this.renameTo(newPath);
		}, RENAME_DELAY);
	}

	syncToPath(path: string): void {
		this.text = fileTitle(path);
	}

	revert(): void {
		this.text = fileTitle(this.host.path());
	}

	blur(): void {
		if (validateName(this.text.trim())) this.revert();
	}

	focusTitle(): void {
		const element = this.host.element();
		if (!element) return;
		element.focus();
		const range = document.createRange();
		range.selectNodeContents(element);
		const selection = window.getSelection();
		if (!selection) return;
		selection.removeAllRanges();
		selection.addRange(range);
	}

	keydown(event: KeyboardEvent): void {
		if (event.key === '/') {
			event.preventDefault();
			this.host.openSlashMenu();
			return;
		}
		if (event.key !== 'Enter' && event.key !== 'ArrowDown') return;
		event.preventDefault();
		this.host.focusEditor();
	}

	async renameTo(newPath: string): Promise<void> {
		if (this.pending) return;
		this.pending = true;
		try {
			const path = this.host.path();
			const differsBeyondCase = newPath.toLowerCase() !== path.toLowerCase();
			if (differsBeyondCase && (await fileExists(newPath))) {
				toast.error(m.toast_path_exists({ name: baseName(newPath) }));
				this.revert();
				return;
			}

			await this.host.onrename()?.(path, newPath);
			this.host.setPath(newPath);
			this.text = fileTitle(newPath);
		} catch (err) {
			console.error('Rename failed:', err);
			this.revert();
			toast.error(m.toast_rename_failed({ error: String(err) }));
		} finally {
			this.pending = false;
		}
	}

	dispose(): void {
		clearTimeout(this.timer);
		this.timer = undefined;
	}
}
