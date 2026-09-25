<script lang="ts">
	import { onDestroy, onMount } from 'svelte';
	import type { Terminal } from '@xterm/xterm';
	import type { FitAddon } from '@xterm/addon-fit';
	import { terminals, type TerminalTab } from '$lib/stores/terminals.svelte';
	import { theme } from '$lib/stores/theme.svelte';
	import { toast } from '$lib/stores/toast.svelte';
	import { ptyResize, ptySpawn, ptyWrite } from '$lib/terminal/bridge';
	import { terminalFontOptions, terminalTheme } from '$lib/terminal/theme';
	import * as m from '$lib/paraglide/messages.js';

	let { tab, visible }: { tab: TerminalTab; visible: boolean } = $props();

	let hostEl = $state<HTMLDivElement | null>(null);
	let term = $state<Terminal | null>(null);
	let fit = $state<FitAddon | null>(null);
	let disposed = false;
	let observer: ResizeObserver | null = null;

	function fitTerminal() {
		const el = hostEl;
		const fitAddon = fit;
		// FitAddon measures this padded host, so the inset stays on it and a hidden 0×0 panel is never fit.
		if (!el || !fitAddon || el.clientWidth === 0 || el.clientHeight === 0) return;
		fitAddon.fit();
	}

	async function copySelection() {
		const text = term?.getSelection() ?? '';
		if (!text) return;
		try {
			await navigator.clipboard.writeText(text);
		} catch (err) {
			console.warn('Failed to copy the terminal selection:', err);
			toast.error(m.terminal_copy_failed());
		}
	}

	onMount(async () => {
		const [{ Terminal }, { FitAddon }] = await Promise.all([
			import('@xterm/xterm'),
			import('@xterm/addon-fit'),
			import('@xterm/xterm/css/xterm.css')
		]);
		if (disposed || !hostEl) return;

		const terminal = new Terminal({
			...terminalFontOptions(),
			theme: terminalTheme(),
			cursorBlink: true,
			scrollback: 5000
		});
		const fitAddon = new FitAddon();
		terminal.loadAddon(fitAddon);
		terminal.open(hostEl);
		term = terminal;
		fit = fitAddon;

		// Ctrl+C copies only a selection; Ctrl+V must fall through to xterm's paste, not ^V.
		terminal.attachCustomKeyEventHandler((event) => {
			if (!event.metaKey && !event.ctrlKey) return true;
			if (event.altKey) return true;
			const key = event.key.toLowerCase();

			if (key === 'c') {
				if (!terminal.hasSelection()) return true;
				void copySelection();
				return false;
			}

			if (key === 'v' && event.ctrlKey && !event.metaKey && !event.shiftKey) return false;

			return true;
		});

		terminal.onData((data) => {
			if (tab.exited !== null || tab.error) return;
			void ptyWrite(tab.id, data).catch(() => {});
		});
		terminal.onResize(({ cols, rows }) => {
			void ptyResize(tab.id, cols, rows).catch(() => {});
		});

		observer = new ResizeObserver(() => fitTerminal());
		observer.observe(hostEl);
		fitTerminal();

		const dims = fitAddon.proposeDimensions();
		try {
			await ptySpawn(tab.id, dims?.cols ?? 80, dims?.rows ?? 24, {
				onOutput: (text) => terminal.write(text),
				onExit: (code) => terminals.markExited(tab.id, code)
			});
		} catch (err) {
			terminals.markFailed(tab.id, err instanceof Error ? err.message : String(err));
		}
	});

	$effect(() => {
		void theme.current;
		if (term) term.options.theme = terminalTheme();
	});

	$effect(() => {
		if (!visible) return;
		fitTerminal();
		term?.focus();
	});

	onDestroy(() => {
		disposed = true;
		observer?.disconnect();
		term?.dispose();
		term = null;
		fit = null;
	});
</script>

<div class="relative h-full w-full bg-background">
	<div
		class="h-full w-full"
		style="padding: var(--space-sm) var(--space-md)"
		bind:this={hostEl}
	></div>

	{#if tab.error}
		<div
			class="absolute inset-0 flex items-center justify-center bg-background px-6 text-center text-xs text-destructive"
		>
			{tab.error}
		</div>
	{:else if tab.exited !== null}
		<div
			class="pointer-events-none absolute inset-x-0 bottom-0 flex items-center gap-2 border-t border-border bg-surface-1 px-3 py-1 text-xs text-subtle-foreground"
		>
			<span class="tabular-nums">{m.terminal_exited({ code: tab.exited })}</span>
		</div>
	{/if}
</div>
