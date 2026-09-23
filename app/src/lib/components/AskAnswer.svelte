<script lang="ts">
	import { onMount } from 'svelte';
	// Type-only, so the editor stack itself stays behind the dynamic import below.
	import type { Editor } from '@tiptap/core';

	interface Props {
		markdown: string;
		oncite: (title: string) => void;
	}

	let { markdown, oncite }: Props = $props();

	let host = $state<HTMLElement | null>(null);
	let editor: Editor | null = null;
	let flushTimer: ReturnType<typeof setTimeout> | null = null;
	let pending = '';

	const FLUSH_MS = 120;

	function flush() {
		flushTimer = null;
		if (!pending) return;
		const text = pending;
		pending = '';
		editor?.commands.setContent(text, { emitUpdate: false });
	}

	function handleClick(event: MouseEvent) {
		const element = (event.target as HTMLElement).closest('[data-wiki-link]');
		const title = element?.getAttribute('data-title');
		if (!title) return;
		event.preventDefault();
		event.stopPropagation();
		oncite(title);
	}

	onMount(() => {
		let disposed = false;
		void (async () => {
			const [
				{ Editor },
				{ default: StarterKit },
				{ Markdown },
				{ default: Link },
				{ default: WikiLink }
			] = await Promise.all([
				import('@tiptap/core'),
				import('@tiptap/starter-kit'),
				import('tiptap-markdown'),
				import('@tiptap/extension-link'),
				import('$lib/editor/wiki-link')
			]);
			if (disposed || !host) return;

			editor = new Editor({
				element: host,
				editable: false,
				extensions: [
					StarterKit.configure({ link: false }),
					Link.configure({ openOnClick: false }),
					WikiLink,
					// `html: false` is the safety property: model output must never become DOM.
					Markdown.configure({
						html: false,
						transformPastedText: false,
						transformCopiedText: false
					})
				],
				content: markdown,
				editorProps: { attributes: { class: 'ask-answer-body' } }
			});
			host.addEventListener('click', handleClick);
		})();

		return () => {
			disposed = true;
			host?.removeEventListener('click', handleClick);
			if (flushTimer) clearTimeout(flushTimer);
			editor?.destroy();
			editor = null;
		};
	});

	$effect(() => {
		const next = markdown;
		if (!editor) return;
		if (flushTimer !== null) {
			pending = next;
			return;
		}
		editor.commands.setContent(next, { emitUpdate: false });
		flushTimer = setTimeout(flush, FLUSH_MS);
	});
</script>

<div class="editor-wrap text-sm text-foreground" bind:this={host}></div>
