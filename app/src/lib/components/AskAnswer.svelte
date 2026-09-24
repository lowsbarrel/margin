<script lang="ts">
	import { onMount } from 'svelte';
	import { vault } from '$lib/stores/vault.svelte';
	import { findVaultFile, vaultFileExists } from '$lib/editor/live/vault-index';
	import { staticPreview } from '$lib/editor/live/context';
	import type { LiveEditorHandle } from '$lib/editor/live/editor-factory';

	interface Props {
		markdown: string;
		oncite: (title: string) => void;
	}

	let { markdown, oncite }: Props = $props();

	let host = $state<HTMLElement | null>(null);
	let handle = $state<LiveEditorHandle | null>(null);

	$effect(() => {
		const text = markdown;
		const editor = handle;
		if (editor) editor.adopt(text);
	});

	onMount(() => {
		let alive = true;
		void (async () => {
			const [{ createLiveEditor }, { EditorState, StateEffect }, { EditorView }] =
				await Promise.all([
					import('$lib/editor/live/editor-factory'),
					import('@codemirror/state'),
					import('@codemirror/view')
				]);
			if (!alive || !host) return;
			const editor = await createLiveEditor({
				parent: host,
				doc: markdown,
				source: false,
				vaultPath: () => vault.vaultPath,
				notePath: () => '',
				attachmentFolder: () => '',
				exists: vaultFileExists,
				findByName: findVaultFile,
				openLightbox: () => {},
				openWikiLink: (title) => oncite(title),
				openContextMenu: () => {},
				onDocChange: () => {},
				onCursor: () => {},
				onSelection: () => {}
			});
			if (!alive) {
				editor.destroy();
				return;
			}
			editor.view.dispatch({
				effects: StateEffect.appendConfig.of([
					staticPreview.of(true),
					EditorState.readOnly.of(true),
					EditorView.editable.of(false),
					EditorView.theme({
						'.cm-content': { padding: '0', margin: '0', maxWidth: 'none' },
						'.cm-scroller': { overflow: 'visible' }
					})
				])
			});
			handle = editor;
		})();
		return () => {
			alive = false;
			handle?.destroy();
			handle = null;
		};
	});
</script>

<div class="editor-wrap ask-answer text-sm text-foreground" bind:this={host}></div>
