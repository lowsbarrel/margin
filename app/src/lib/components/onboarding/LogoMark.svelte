<script lang="ts">
	import { motion } from '$lib/stores/motion.svelte';

	interface Props {
		size?: number;
	}

	let { size = 64 }: Props = $props();
</script>

<!-- Draws itself on mount: the frame first, then the mark, then the pen dot.
     `pathLength="1"` normalises the dash maths so the two strokes can't drift. -->
<svg
	xmlns="http://www.w3.org/2000/svg"
	width={size}
	height={size}
	viewBox="0 0 64 64"
	fill="none"
	stroke="currentColor"
	stroke-width="4"
	stroke-linecap="round"
	stroke-linejoin="round"
	class="mark"
	class:reduced={motion.reduced}
	aria-hidden="true"
>
	<rect class="frame" x="6" y="6" width="52" height="52" rx="15" pathLength="1" />
	<path class="swoosh" d="M18 46V20l14 20 14-20v26" pathLength="1" />
	<circle class="dot" cx="50" cy="14" r="3.5" fill="currentColor" stroke="none" />
</svg>

<style>
	.frame,
	.swoosh {
		stroke-dasharray: 1;
		stroke-dashoffset: 1;
	}

	.mark:not(.reduced) .frame {
		animation: draw 800ms var(--ease-out) 100ms both;
	}

	.mark:not(.reduced) .swoosh {
		animation: draw 700ms var(--ease-out) 550ms both;
	}

	.dot {
		transform-origin: 50px 14px;
	}

	.mark:not(.reduced) .dot {
		animation: pop 320ms cubic-bezier(0.2, 1.4, 0.4, 1) 1150ms both;
	}

	.mark.reduced .frame,
	.mark.reduced .swoosh {
		stroke-dashoffset: 0;
	}

	@keyframes draw {
		to {
			stroke-dashoffset: 0;
		}
	}

	@keyframes pop {
		from {
			transform: scale(0);
			opacity: 0;
		}
		to {
			transform: scale(1);
			opacity: 1;
		}
	}
</style>
