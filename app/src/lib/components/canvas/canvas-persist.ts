import { serialize } from '$lib/canvas/serialization';
import { writeFileBytes } from '$lib/fs/bridge';
import { editor } from '$lib/stores/editor.svelte';
import { toast } from '$lib/stores/toast.svelte';
import * as m from '$lib/paraglide/messages.js';
import type { CanvasCamera } from './canvas-camera.svelte';
import type { CanvasScene } from './canvas-scene';

const PERSIST_DEBOUNCE_MS = 400;

interface PersistDeps {
	path: () => string;
	scene: CanvasScene;
	camera: CanvasCamera;
	onsave: () => ((content: string) => void) | undefined;
}

export interface CanvasPersist {
	flush: () => void;
	schedule: () => void;
}

export function createCanvasPersist({ path, scene, camera, onsave }: PersistDeps): CanvasPersist {
	let timer: ReturnType<typeof setTimeout> | null = null;
	let pending = false;

	function flush() {
		if (timer !== null) {
			clearTimeout(timer);
			timer = null;
		}
		if (!pending) return;
		pending = false;
		const data = serialize(
			scene.strokes,
			scene.shapes,
			scene.labels,
			camera.x,
			camera.y,
			camera.zoom
		);
		onsave()?.(data);
		editor.markLocalChange();
		writeFileBytes(path(), new TextEncoder().encode(data)).catch((err) => {
			console.error('Canvas save failed:', err);
			toast.error(m.toast_canvas_save_failed());
		});
	}

	function schedule() {
		pending = true;
		if (timer !== null) clearTimeout(timer);
		timer = setTimeout(flush, PERSIST_DEBOUNCE_MS);
	}

	return { flush, schedule };
}
