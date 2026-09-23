import type { Editor } from '@tiptap/core';
import { buildEditorContextMenu, handleEditorClick } from '$lib/editor/handlers/clicks';
import type { ContextMenuItem } from '$lib/components/ContextMenu.svelte';
import type { LightboxImage } from '$lib/components/ImageLightbox.svelte';

export interface InteractionHost {
	container(): HTMLElement | undefined;
	rich(): Editor | null;
	vaultPath(): string | null;
	onWikiLink(): ((title: string) => void) | undefined;
}

export class EditorInteractions {
	private readonly host: InteractionHost;
	lightbox = $state<{ images: LightboxImage[]; index: number } | null>(null);
	contextMenu = $state<{ x: number; y: number; items: ContextMenuItem[] } | null>(null);

	constructor(host: InteractionHost) {
		this.host = host;
	}

	openLightbox = (src: string, alt: string): void => {
		const container = this.host.container();
		const images: LightboxImage[] = container
			? Array.from(container.querySelectorAll('img')).map((img) => ({
					src: img.src,
					alt: img.alt
				}))
			: [];
		const clicked = images.findIndex((image) => image.src === src);
		this.lightbox =
			clicked >= 0 ? { images, index: clicked } : { images: [{ src, alt }], index: 0 };
	};

	navigateLightbox(index: number): void {
		if (this.lightbox) this.lightbox = { ...this.lightbox, index };
	}

	linkClick = (event: MouseEvent): void => {
		const container = this.host.container();
		if (!container) return;
		handleEditorClick(event, container, {
			vaultPath: this.host.vaultPath(),
			onLightbox: this.openLightbox,
			onWikiLink: this.host.onWikiLink()
		});
	};

	editorContextMenu = (event: MouseEvent): void => {
		const container = this.host.container();
		if (!container) return;
		const result = buildEditorContextMenu(event, container, this.host.rich(), {
			vaultPath: this.host.vaultPath(),
			onLightbox: this.openLightbox
		});
		if (result) this.contextMenu = result;
	};
}
