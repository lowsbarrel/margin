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
	private queued: string | null = null;
	private timer: ReturnType<typeof setTimeout> | undefined;

	constructor(host: TitleEditorHost, initialTitle: string) {
		this.host = host;
		this.text = untrack(() => initialTitle);
	}

	input(raw: string): void {
		clearTimeout(this.timer);
		this.timer = undefined;
		if (!this.host.isAlive() || !raw) return;
		this.timer = setTimeout(() => {
			this.timer = undefined;
			this.request(raw, true);
		}, RENAME_DELAY);
	}

	// Writing the title while it has focus replaces its text node and throws the caret to the start.
	syncToPath(path: string): void {
		if (!this.editing()) this.text = fileTitle(path);
	}

	blur(): void {
		const debounced = this.timer !== undefined;
		clearTimeout(this.timer);
		this.timer = undefined;
		this.request(this.text.trim(), debounced);
		if (!this.pending) this.text = fileTitle(this.host.path());
	}

	private editing(): boolean {
		const element = this.host.element();
		return !!element && element === document.activeElement;
	}

	private request(name: string, report: boolean): void {
		if (!this.host.isAlive() || !name) return;
		const error = validateName(name);
		if (error) {
			if (report) toast.error(error);
			return;
		}
		this.queued = name;
		void this.drain();
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

	private async drain(): Promise<void> {
		if (this.pending) return;
		this.pending = true;
		try {
			while (this.queued !== null && this.host.isAlive()) {
				const name = this.queued;
				this.queued = null;
				const path = this.host.path();
				if (name !== fileTitle(path)) await this.rename(path, `${parentDir(path)}/${name}.md`);
			}
		} finally {
			this.pending = false;
			if (this.host.isAlive() && !this.editing()) this.text = fileTitle(this.host.path());
		}
	}

	private async rename(path: string, newPath: string): Promise<void> {
		try {
			const differsBeyondCase = newPath.toLowerCase() !== path.toLowerCase();
			if (differsBeyondCase && (await fileExists(newPath))) {
				toast.error(m.toast_path_exists({ name: baseName(newPath) }));
				return;
			}
			await this.host.onrename()?.(path, newPath);
			this.host.setPath(newPath);
		} catch (err) {
			console.error('Rename failed:', err);
			toast.error(m.toast_rename_failed({ error: String(err) }));
		}
	}

	dispose(): void {
		clearTimeout(this.timer);
		this.timer = undefined;
	}
}
