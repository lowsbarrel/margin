<script lang="ts">
	import { motion } from '$lib/stores/motion.svelte';

	const TILTS = [-10, 0, 10];
</script>

<div class="art" class:reduced={motion.reduced}>
	<svg viewBox="0 0 200 130" aria-hidden="true">
		{#each TILTS as tilt, i (i)}
			<g class="version" style="--i: {i}; --tilt: {tilt}">
				<rect class="sheet" x="44" y="40" width="54" height="68" rx="5" />
				<line class="sheet-line" x1="56" y1="56" x2="86" y2="56" />
				<line class="sheet-line" x1="56" y1="66" x2="80" y2="66" />
			</g>
		{/each}

		<g class="clock">
			<circle class="clock-face" cx="176" cy="34" r="11" />
			<path class="clock-hand" d="M176 34v-5M176 34h4" />
		</g>

		<line class="bin-lid" x1="126" y1="66" x2="178" y2="66" />
		<path class="bin-handle" d="M144 60h16" />
		<path class="bin" d="M131 66l4 49a4 4 0 0 0 4 3h26a4 4 0 0 0 4-3l4-49" />
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

	.sheet {
		fill: var(--color-bg-secondary);
		stroke: var(--color-border-strong);
		stroke-width: 1.5;
	}

	.version {
		transform-origin: 71px 108px;
		transform: rotate(calc(var(--tilt) * 1deg));
		animation: fan 480ms var(--ease-out) both;
		animation-delay: calc(120ms + var(--i) * 150ms);
	}

	.sheet-line {
		stroke: var(--color-border-strong);
		stroke-width: 1.5;
		stroke-linecap: round;
	}

	.clock-face,
	.clock-hand {
		fill: none;
		stroke: var(--color-text-tertiary);
		stroke-width: 2;
		stroke-linecap: round;
	}

	.clock {
		transform-origin: 176px 34px;
		animation: tick 700ms var(--ease-out) 620ms both;
	}

	.bin,
	.bin-lid,
	.bin-handle {
		fill: none;
		stroke: var(--color-text-brand);
		stroke-width: 2.5;
		stroke-linecap: round;
		stroke-linejoin: round;
		stroke-dasharray: 1;
		stroke-dashoffset: 1;
		animation: draw 460ms var(--ease-out) both;
	}

	.bin {
		animation-delay: 700ms;
	}

	.bin-lid {
		animation-delay: 1020ms;
	}

	.bin-handle {
		animation-delay: 1160ms;
	}

	.reduced .version,
	.reduced .clock,
	.reduced .bin,
	.reduced .bin-lid,
	.reduced .bin-handle {
		animation: none;
	}

	.reduced .bin,
	.reduced .bin-lid,
	.reduced .bin-handle {
		stroke-dashoffset: 0;
	}

	@keyframes fan {
		from {
			transform: rotate(0deg);
			opacity: 0;
		}
		to {
			transform: rotate(calc(var(--tilt) * 1deg));
			opacity: 1;
		}
	}

	@keyframes tick {
		from {
			transform: rotate(-90deg);
			opacity: 0;
		}
		to {
			transform: rotate(0deg);
			opacity: 1;
		}
	}

	@keyframes draw {
		to {
			stroke-dashoffset: 0;
		}
	}
</style>
