<script lang="ts">
	import { motion } from '$lib/stores/motion.svelte';

	/* Skeleton bars, not real words: twelve of them communicates "a passphrase
	   long enough to write down" without looking like a phrase to copy. */
	const WORDS = Array.from({ length: 12 }, (_, i) => ({
		x: 7 + (i % 4) * 48,
		y: 14 + Math.floor(i / 4) * 22
	}));
</script>

<div class="art" class:reduced={motion.reduced}>
	<svg viewBox="0 0 200 130" aria-hidden="true">
		{#each WORDS as w, i (i)}
			<rect class="word" x={w.x} y={w.y} width="42" height="16" rx="4" style="--i: {i}" />
		{/each}
		<circle class="key-ring" cx="72" cy="104" r="8" pathLength="1" />
		<path class="key-shaft" d="M80 104h48" pathLength="1" />
		<path class="key-teeth" d="M112 104v-8M120 104v-8" pathLength="1" />
	</svg>
</div>

<style>
	.art {
		width: 100%;
	}

	svg {
		display: block;
		width: 100%;
		height: auto;
	}

	.word {
		fill: var(--color-bg-secondary);
		stroke: var(--color-border-strong);
		stroke-width: 1.5;
		animation: write 380ms var(--ease-out) both;
		animation-delay: calc(60ms + var(--i) * 65ms);
	}

	.key-ring,
	.key-shaft,
	.key-teeth {
		fill: none;
		stroke: var(--color-text-brand);
		stroke-width: 3;
		stroke-linecap: round;
		stroke-dasharray: 1;
		stroke-dashoffset: 1;
		animation: draw 420ms var(--ease-out) both;
	}

	.key-ring {
		animation-delay: 900ms;
	}

	.key-shaft {
		animation-delay: 1080ms;
	}

	.key-teeth {
		animation-delay: 1240ms;
	}

	.reduced .word,
	.reduced .key-ring,
	.reduced .key-shaft,
	.reduced .key-teeth {
		animation: none;
	}

	.reduced .key-ring,
	.reduced .key-shaft,
	.reduced .key-teeth {
		stroke-dashoffset: 0;
	}

	@keyframes write {
		from {
			transform: translateY(9px);
			opacity: 0;
		}
		to {
			transform: translateY(0);
			opacity: 1;
		}
	}

	@keyframes draw {
		to {
			stroke-dashoffset: 0;
		}
	}
</style>
