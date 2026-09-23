<script lang="ts">
	import { motion } from '$lib/stores/motion.svelte';

	const CARDS = [58, 84, 110];
</script>

<!-- Three notes drop into the folder one after another, then the front panel
     rises over their lower edge so they read as filed away. -->
<div class="art" class:reduced={motion.reduced}>
	<svg viewBox="0 0 200 130" aria-hidden="true">
		<path
			class="folder"
			d="M33 78a7 7 0 0 1 7-7h38l9 9h76a7 7 0 0 1 7 7v27a7 7 0 0 1-7 7H40a7 7 0 0 1-7-7z"
		/>
		{#each CARDS as x, i (x)}
			<g class="card" style="--i: {i}">
				<rect class="card-body" {x} y="62" width="34" height="46" rx="4" />
				<line class="card-line" x1={x + 7} y1="74" x2={x + 27} y2="74" />
				<line class="card-line" x1={x + 7} y1="82" x2={x + 24} y2="82" />
				<line class="card-line" x1={x + 7} y1="90" x2={x + 27} y2="90" />
			</g>
		{/each}
		<path class="folder-front" d="M33 96h134v18a7 7 0 0 1-7 7H40a7 7 0 0 1-7-7z" />
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

	.folder {
		fill: var(--color-bg-secondary);
		stroke: var(--color-border-strong);
		stroke-width: 1.5;
	}

	.folder-front {
		fill: var(--color-bg-secondary);
		stroke: var(--color-border-strong);
		stroke-width: 1.5;
		transform-origin: 100px 121px;
		animation: close 320ms var(--ease-out) 980ms both;
	}

	.card-body {
		fill: var(--color-bg-primary);
		stroke: var(--color-border-strong);
		stroke-width: 1.5;
	}

	.card-line {
		stroke: var(--color-border-strong);
		stroke-width: 1.5;
	}

	.card {
		animation: drop 620ms var(--ease-out) both;
		animation-delay: calc(120ms + var(--i) * 170ms);
	}

	.reduced .card,
	.reduced .folder-front {
		animation: none;
	}

	@keyframes drop {
		from {
			transform: translateY(-34px);
			opacity: 0;
		}
		to {
			transform: translateY(0);
			opacity: 1;
		}
	}

	@keyframes close {
		from {
			transform: scaleY(0.35);
			opacity: 0;
		}
		to {
			transform: scaleY(1);
			opacity: 1;
		}
	}
</style>
