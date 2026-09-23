<script lang="ts">
	/**
	 * Renders a streamed answer as Markdown, read-only.
	 *
	 * The answer is model output, so it is never injected as HTML: markdown-it
	 * runs with raw HTML disabled, ProseMirror turns the parsed document into
	 * DOM, and nothing the model writes can produce a tag of its own. `[[cites]]`
	 * still arrive as the editor's wiki-link nodes, so a click can open the note
	 * they name.
	 */
	import { onMount } from 'svelte';
	// Type-only: erased at build time, so the editor itself stays a lazy import.
	import type { Editor } from '@tiptap/core';

	interface Props {
		markdown: string;
		/** A cited note title, e.g. `Roadmap` from `[[Roadmap]]`. */
		oncite: (title: string) => void;
	}

	let { markdown, oncite }: Props = $props();

	let host = $state<HTMLElement | null>(null);
	let editor: Editor | null = null;
	let flushTimer: ReturnType<typeof setTimeout> | null = null;
	/** The newest text waiting for the next repaint. */
	let pending = '';

	/** Stream deltas arrive per token; the document is rebuilt at most this often. */
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
		// The editor stack is browser-only and heavy; the palette pulls it in only
		// once there is an answer to draw.
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
					// `html: false` is the safety property: raw HTML in the answer
					// stays text instead of becoming DOM.
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
		// Leading edge first, so text appears as soon as it starts arriving, then
		// one repaint per FLUSH_MS regardless of how fast the deltas come.
		if (flushTimer !== null) {
			pending = next;
			return;
		}
		editor.commands.setContent(next, { emitUpdate: false });
		flushTimer = setTimeout(flush, FLUSH_MS);
	});
</script>

<div class="editor-wrap text-sm text-foreground" bind:this={host}></div>
