<script lang="ts">
	interface IconProps {
		color?: string;
		size?: number;
		strokeWidth?: number;
		animate?: boolean;
		class?: string;
	}

	let {
		color = 'currentColor',
		size = 24,
		strokeWidth = 2,
		animate: animateProp = false,
		class: className = ''
	}: IconProps = $props();

	let hoverAnimate = $state(false);
	let resetTimer: ReturnType<typeof setTimeout> | undefined;
	const animate = $derived(animateProp || hoverAnimate);

	function handleMouseEnter() {
		if (animate) return;
		hoverAnimate = true;

		resetTimer = setTimeout(() => {
			hoverAnimate = false;
		}, 300);
	}

	$effect(() => () => clearTimeout(resetTimer));
</script>

<div class={className} aria-label="panel-left" role="img" onmouseenter={handleMouseEnter}>
	<svg
		xmlns="http://www.w3.org/2000/svg"
		width={size}
		height={size}
		viewBox="0 0 24 24"
		fill="none"
		stroke={color}
		stroke-width={strokeWidth}
		stroke-linecap="round"
		stroke-linejoin="round"
	>
		<rect width="18" height="18" x="3" y="3" rx="2" />
		<path d="M9 3v18" class:line={animate} />
	</svg>
</div>

<style>
	div {
		/* Local change from the upstream registry: inline-flex instead of
		   inline-block, so the wrapper adds no line-height gap when it sits
		   inside margin's flex button rows. */
		display: inline-flex;
		align-items: center;
		justify-content: center;
		flex-shrink: 0;
	}
	path {
		transition: all 0.2s ease-in;
	}

	.line {
		transform: translateX(-2px);
		transition-delay: 0.05s;
	}
</style>
