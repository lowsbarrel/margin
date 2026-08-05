<script lang="ts">
	import type { Editor } from '@tiptap/core';
	import { ChevronDown } from '@lucide/svelte';
	import * as m from '$lib/paraglide/messages.js';

	/**
	 * Shared utility strings for the toolbar's buttons. `is-active` is toggled
	 * imperatively from `updateActiveStates()` (and from the editor's bubble-menu
	 * handler), so its styling has to hang off the class itself rather than a
	 * Svelte `class:` directive.
	 *
	 * The `!` modifiers exist because `src/app.css` styles bare `button`,
	 * `input` and `select` outside any cascade layer, which outranks every
	 * Tailwind utility (utilities live in `@layer utilities`). Only the
	 * properties those element rules actually set need it.
	 */
	const TOOLBAR_BTN =
		'flex size-7 items-center justify-center rounded-xs p-0 text-muted-foreground transition-colors hover:bg-surface-3 hover:text-foreground [&.is-active]:bg-accent [&.is-active]:text-accent-foreground';
	const LINK_BTN =
		'flex size-6.5 items-center justify-center rounded-xs p-0 text-muted-foreground transition-colors hover:bg-surface-3';

	interface Props {
		editor: Editor | null;
	}

	let { editor }: Props = $props();
	let toolbarEl: HTMLDivElement;
	let showLinkInput = $state(false);
	let linkUrl = $state('');
	let linkInputEl = $state<HTMLInputElement>();
	let cachedButtons: HTMLButtonElement[] = [];

	function cacheButtons() {
		if (!toolbarEl) return;
		cachedButtons = Array.from(toolbarEl.querySelectorAll<HTMLButtonElement>('[data-cmd]'));
	}

	function updateActiveStates() {
		if (!editor || !toolbarEl) return;
		if (cachedButtons.length === 0) cacheButtons();
		for (const btn of cachedButtons) {
			const cmd = btn.dataset.cmd!;
			btn.classList.toggle('is-active', editor!.isActive(cmd));
		}
	}

	function handleClick(e: MouseEvent) {
		const btn = (e.target as HTMLElement).closest<HTMLButtonElement>('[data-cmd]');
		if (!btn || !editor) return;
		const cmd = btn.dataset.cmd!;
		switch (cmd) {
			case 'bold':
				editor.chain().focus().toggleBold().run();
				break;
			case 'italic':
				editor.chain().focus().toggleItalic().run();
				break;
			case 'strike':
				editor.chain().focus().toggleStrike().run();
				break;
			case 'code':
				editor.chain().focus().toggleMark('code').run();
				break;

			case 'link':
				if (editor.isActive('link')) {
					editor.chain().focus().unsetLink().run();
				} else {
					showLinkInput = true;
					linkUrl = '';
					requestAnimationFrame(() => linkInputEl?.focus());
				}
				break;
		}
		updateActiveStates();
	}

	function submitLink() {
		if (linkUrl.trim() && editor) {
			editor.chain().focus().setLink({ href: linkUrl.trim() }).run();
		}
		showLinkInput = false;
		linkUrl = '';
		cachedButtons = [];
		updateActiveStates();
	}

	function cancelLink() {
		showLinkInput = false;
		linkUrl = '';
		cachedButtons = [];
		editor?.commands.focus();
	}

	function handleLinkKeydown(e: KeyboardEvent) {
		if (e.key === 'Enter') {
			e.preventDefault();
			submitLink();
		} else if (e.key === 'Escape') {
			e.preventDefault();
			cancelLink();
		}
	}

	function getCurrentHeadingLevel(ed: Editor): number | null {
		for (let l = 1; l <= 6; l++) {
			if (ed.isActive('heading', { level: l })) return l;
		}
		return null;
	}

	function getActiveBlockType(ed: Editor): string {
		const lvl = getCurrentHeadingLevel(ed);
		if (lvl !== null) return `h${lvl}`;
		if (ed.isActive('bulletList')) return 'bullet';
		if (ed.isActive('orderedList')) return 'ordered';
		if (ed.isActive('taskList')) return 'task';
		if (ed.isActive('blockquote')) return 'quote';
		if (ed.isActive('codeBlock')) return 'code';
		if (ed.isActive('callout')) return 'callout';
		if (ed.isActive('details')) return 'details';
		return 'text';
	}

	let blockType = $derived.by(() => {
		if (!editor) return 'text';
		return getActiveBlockType(editor);
	});

	function handleBlockChange(e: Event) {
		if (!editor) return;
		const val = (e.target as HTMLSelectElement).value;
		const chain = editor.chain().focus();
		switch (val) {
			case 'text':
				chain.setParagraph().run();
				break;
			case 'h1':
				chain.setHeading({ level: 1 }).run();
				break;
			case 'h2':
				chain.setHeading({ level: 2 }).run();
				break;
			case 'h3':
				chain.setHeading({ level: 3 }).run();
				break;
			case 'h4':
				chain.setHeading({ level: 4 }).run();
				break;
			case 'h5':
				chain.setHeading({ level: 5 }).run();
				break;
			case 'h6':
				chain.setHeading({ level: 6 }).run();
				break;
			case 'bullet':
				chain.toggleBulletList().run();
				break;
			case 'ordered':
				chain.toggleOrderedList().run();
				break;
			case 'task':
				chain.toggleTaskList().run();
				break;
			case 'quote':
				chain.toggleBlockquote().run();
				break;
			case 'code':
				chain.toggleCodeBlock().run();
				break;
			case 'callout':
				chain.toggleCallout({ type: 'info' }).run();
				break;
			case 'details':
				chain.setDetails().run();
				break;
		}
		updateActiveStates();
	}

	export { updateActiveStates };
</script>

<!-- svelte-ignore a11y_click_events_have_key_events -->
<!-- svelte-ignore a11y_no_static_element_interactions -->
<div
	class="surface-popover flex items-center gap-0.5 px-1.5 py-1"
	bind:this={toolbarEl}
	onclick={handleClick}
>
	{#if showLinkInput}
		<div class="flex items-center gap-1">
			<input
				bind:this={linkInputEl}
				bind:value={linkUrl}
				class="h-6.5 w-50 rounded-xs border border-border bg-surface-2 px-2 py-0 font-mono text-sm text-foreground caret-foreground outline-none"
				type="url"
				placeholder="https://..."
				onkeydown={handleLinkKeydown}
			/>
			<button
				class="{LINK_BTN} hover:text-positive"
				onclick={submitLink}
				aria-label={m.bubble_confirm_link()}
			>
				<svg
					width="14"
					height="14"
					viewBox="0 0 24 24"
					fill="none"
					stroke="currentColor"
					stroke-width="2"><polyline points="20 6 9 17 4 12" /></svg
				>
			</button>
			<button
				class="{LINK_BTN} hover:text-destructive"
				onclick={cancelLink}
				aria-label={m.bubble_cancel_link()}
			>
				<svg
					width="14"
					height="14"
					viewBox="0 0 24 24"
					fill="none"
					stroke="currentColor"
					stroke-width="2"
					><line x1="18" y1="6" x2="6" y2="18" /><line x1="6" y1="6" x2="18" y2="18" /></svg
				>
			</button>
		</div>
	{:else}
		<button class={TOOLBAR_BTN} data-cmd="bold" title={m.bubble_bold()}><b>B</b></button>
		<button class={TOOLBAR_BTN} data-cmd="italic" title={m.bubble_italic()}><i>I</i></button>
		<button class={TOOLBAR_BTN} data-cmd="strike" title={m.bubble_strike()}><s>S</s></button>
		<button class={TOOLBAR_BTN} data-cmd="code" title={m.bubble_code()}>
			<code class="font-mono text-xs">&lt;/&gt;</code>
		</button>
		<span class="mx-[3px] h-4.5 w-px bg-border"></span>
		<span class="relative flex items-center">
			<select
				class="h-6.5 cursor-pointer appearance-none rounded-xs border border-border bg-surface-2 py-0 pr-4.5 pl-1.5 text-xs text-foreground hover:border-input"
				value={blockType}
				onchange={handleBlockChange}
				title={m.bubble_block_type()}
			>
				<option value="text">{m.bubble_text()}</option>
				<option value="h1">H1</option>
				<option value="h2">H2</option>
				<option value="h3">H3</option>
				<option value="h4">H4</option>
				<option value="h5">H5</option>
				<option value="h6">H6</option>
				<option value="bullet">• Bullet list</option>
				<option value="ordered">1. Numbered list</option>
				<option value="task">☑ To-do list</option>
				<option value="quote">" Quote</option>
				<option value="code">&lt;/&gt; Code block</option>
				<option value="callout">💡 Callout</option>
				<option value="details">▶ Toggle</option>
			</select>
			<ChevronDown size={12} class="pointer-events-none absolute right-1 text-muted-foreground" />
		</span>
		<span class="mx-[3px] h-4.5 w-px bg-border"></span>
		<button class={TOOLBAR_BTN} data-cmd="link" title={m.bubble_link()}>
			<svg
				width="16"
				height="16"
				viewBox="0 0 24 24"
				fill="none"
				stroke="currentColor"
				stroke-width="2"
				><path d="M10 13a5 5 0 0 0 7.54.54l3-3a5 5 0 0 0-7.07-7.07l-1.72 1.71" /><path
					d="M14 11a5 5 0 0 0-7.54-.54l-3 3a5 5 0 0 0 7.07 7.07l1.71-1.71"
				/></svg
			>
		</button>
	{/if}
</div>

<style>
	/* Everything else on this toolbar is expressed as utilities in the markup.
	   This rule can't be: the class is attached by a ProseMirror decoration
	   (`$lib/editor/selection-preserve.ts`) to text nodes the editor owns, and
	   the two values are system colours with no token equivalent. */
	:global(.selection-preserved) {
		background: Highlight;
		color: HighlightText;
	}
</style>
