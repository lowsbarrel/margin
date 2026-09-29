<script lang="ts">
	import { motion } from '$lib/stores/motion.svelte';
</script>

<div class="art" class:reduced={motion.reduced}>
	<svg viewBox="0 0 200 130" aria-hidden="true">
		<rect class="note" x="22" y="42" width="46" height="54" rx="6" />
		<line class="note-line" x1="32" y1="58" x2="58" y2="58" />
		<line class="note-line" x1="32" y1="68" x2="52" y2="68" />
		<line class="note-line" x1="32" y1="78" x2="58" y2="78" />

		<path class="bucket" d="M136 52v38c0 4 10.7 7 24 7s24-3 24-7V52" />
		<ellipse class="bucket" cx="160" cy="52" rx="24" ry="7" />

		<path class="arrow" d="M74 72h50" pathLength="1" />
		<path class="arrow-head" d="M130 65l9 7-9 7" pathLength="1" />

		<g class="lock">
			<rect class="lock-body" x="88" y="66" width="22" height="20" rx="3" />
			<path class="lock-shackle" d="M93 66v-7a6 6 0 0 1 12 0v7" pathLength="1" />
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

	.note {
		fill: var(--color-bg-secondary);
		stroke: var(--color-border-strong);
		stroke-width: 1.5;
	}

	.note-line {
		stroke: var(--color-border-strong);
		stroke-width: 1.5;
	}

	.bucket {
		fill: var(--color-bg-secondary);
		stroke: var(--color-border-strong);
		stroke-width: 1.5;
	}

	.arrow,
	.arrow-head {
		fill: none;
		stroke: var(--color-text-tertiary);
		stroke-width: 2;
		stroke-linecap: round;
		stroke-linejoin: round;
		stroke-dasharray: 1;
		stroke-dashoffset: 1;
		animation: draw 420ms var(--ease-out) both;
	}

	.arrow-head {
		animation-delay: 360ms;
	}

	.lock {
		transform-origin: 99px 86px;
		animation: seal 420ms cubic-bezier(0.2, 1.2, 0.4, 1) 700ms both;
	}

	.lock-body {
		fill: var(--color-bg-primary);
		stroke: var(--color-text-brand);
		stroke-width: 2.5;
	}

	.lock-shackle {
		fill: none;
		stroke: var(--color-text-brand);
		stroke-width: 2.5;
		stroke-linecap: round;
		stroke-dasharray: 1;
		stroke-dashoffset: 1;
		animation: draw 380ms var(--ease-out) 980ms both;
	}

	.reduced .arrow,
	.reduced .arrow-head,
	.reduced .lock,
	.reduced .lock-shackle {
		animation: none;
	}

	.reduced .arrow,
	.reduced .arrow-head,
	.reduced .lock-shackle {
		stroke-dashoffset: 0;
	}

	@keyframes draw {
		to {
			stroke-dashoffset: 0;
		}
	}

	@keyframes seal {
		from {
			transform: translateY(-26px);
			opacity: 0;
		}
		to {
			transform: translateY(0);
			opacity: 1;
		}
	}
</style>
