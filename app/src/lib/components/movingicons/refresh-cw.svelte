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
	const animate = $derived(animateProp || hoverAnimate);

	function handleMouseEnter() {
		hoverAnimate = true;
	}

	function handleMouseLeave() {
		hoverAnimate = false;
	}
</script>

<div
	class={className}
	aria-label="refresh-cw"
	role="img"
	onmouseenter={handleMouseEnter}
	onmouseleave={handleMouseLeave}
>
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
		class="refresh-cw-icon"
		class:animate
	>
		<path d="M3 12a9 9 0 0 1 9-9 9.75 9.75 0 0 1 6.74 2.74L21 8" />
		<path d="M21 3v5h-5" />
		<path d="M21 12a9 9 0 0 1-9 9 9.75 9.75 0 0 1-6.74-2.74L3 16" />
		<path d="M8 16H3v5" />
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
	.refresh-cw-icon {
		transition: transform 0.4s cubic-bezier(0.175, 0.885, 0.32, 1.275);
		transform-origin: center;
	}
	.refresh-cw-icon.animate {
		transform: rotate(50deg);
	}

	svg {
		overflow: visible;
	}
</style>
