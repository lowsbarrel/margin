<script lang="ts">
	import { motion } from '$lib/stores/motion.svelte';
</script>

<!-- Spotlight: the query types itself, then the matches land — a `#tag` row and
     an `?ask` row, the two things the palette does beyond plain search. -->
<div class="art" class:reduced={motion.reduced}>
	<svg viewBox="0 0 200 130" aria-hidden="true">
		<rect class="palette" x="14" y="18" width="172" height="94" rx="8" />
		<circle class="lens" cx="34" cy="42" r="7" />
		<path class="lens" d="M39 47l6 6" pathLength="1" />
		<rect class="bar" x="50" y="38" width="34" height="8" rx="4" style="--i: 0" />
		<rect class="bar" x="90" y="38" width="20" height="8" rx="4" style="--i: 1" />
		<line class="caret" x1="116" y1="34" x2="116" y2="50" />
		<line class="divider" x1="26" y1="58" x2="174" y2="58" />

		<g class="row" style="--r: 0">
			<rect class="chip" x="26" y="70" width="12" height="12" rx="3" />
			<line class="row-line" x1="48" y1="74" x2="150" y2="74" />
			<line class="row-line" x1="48" y1="84" x2="112" y2="84" />
		</g>
		<g class="row" style="--r: 1">
			<rect class="chip chip-ask" x="26" y="94" width="12" height="12" rx="3" />
			<line class="row-line" x1="48" y1="98" x2="130" y2="98" />
		</g>
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

	.palette {
		fill: var(--color-bg-secondary);
		stroke: var(--color-border-strong);
		stroke-width: 1.5;
	}

	.lens {
		fill: none;
		stroke: var(--color-text-tertiary);
		stroke-width: 2;
		stroke-linecap: round;
	}

	.bar {
		fill: var(--color-bg-primary);
		stroke: var(--color-border-strong);
		stroke-width: 1.5;
		animation: type 260ms var(--ease-out) both;
		animation-delay: calc(180ms + var(--i) * 220ms);
	}

	.caret {
		stroke: var(--color-text-brand);
		stroke-width: 2;
		stroke-linecap: round;
		animation: blink 1.1s steps(1, end) 820ms infinite;
	}

	.divider {
		stroke: var(--color-border-secondary);
		stroke-width: 1.5;
	}

	.chip {
		fill: var(--color-brand-24);
	}

	.chip-ask {
		fill: var(--color-brand-16);
	}

	.row-line {
		stroke: var(--color-border-strong);
		stroke-width: 1.5;
		stroke-linecap: round;
	}

	.row {
		animation: land 380ms var(--ease-out) both;
		animation-delay: calc(760ms + var(--r) * 200ms);
	}

	.reduced .bar,
	.reduced .row,
	.reduced .caret {
		animation: none;
	}

	@keyframes type {
		from {
			transform: translateX(-8px);
			opacity: 0;
		}
		to {
			transform: translateX(0);
			opacity: 1;
		}
	}

	@keyframes land {
		from {
			transform: translateY(7px);
			opacity: 0;
		}
		to {
			transform: translateY(0);
			opacity: 1;
		}
	}

	@keyframes blink {
		0%,
		49% {
			opacity: 1;
		}
		50%,
		100% {
			opacity: 0;
		}
	}
</style>
