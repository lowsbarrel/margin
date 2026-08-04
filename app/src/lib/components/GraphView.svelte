<script lang="ts">
	import { onMount, onDestroy } from 'svelte';
	import { graph } from '$lib/stores/graph.svelte';
	// The graph's plain lookup structures live in a non-reactive module on
	// purpose — see the header of render-state.ts.
	import {
		buildFlatGraph,
		computeHighlights,
		snapshotPositions,
		type GraphInput,
		type SeedFn
	} from '$lib/graph/render-state';
	import { files } from '$lib/stores/files.svelte';
	import { theme } from '$lib/stores/theme.svelte';
	import { RefreshCw } from '@lucide/svelte';

	interface Props {
		onfileselect: (path: string) => void;
	}

	let { onfileselect }: Props = $props();

	let canvas = $state<HTMLCanvasElement | null>(null);
	let container = $state<HTMLDivElement | null>(null);
	let rafId = 0;
	let mounted = false;

	// The physics simulation runs in a Web Worker; this component only renders
	// and handles interaction. `workerActive` mirrors whether the worker is still
	// ticking (false once it reports the layout has settled), which — together
	// with an active drag — keeps the render loop alive.
	let worker: Worker | null = null;
	let workerActive = false;

	// Cap the simulated graph to this many nodes. Physics now runs off the main
	// thread, so this can be far higher than the old main-thread limit of 500 —
	// the only remaining per-frame main-thread cost is drawing (O(nodes+edges)).
	// On larger vaults, the most-connected nodes are kept.
	const MAX_SIM_NODES = 2000;
	let totalNodeCount = $state(0); // full count before cap, for UI badge

	/**
	 * The graph is drawn to a canvas, so it cannot inherit CSS — every colour has
	 * to be resolved to a string first. These used to be hardcoded rgba() literals
	 * tuned for the dark theme, which meant the graph ignored the design tokens
	 * entirely and looked wrong on a light background.
	 *
	 * Resolving them from the same `--color-*` tokens everything else uses keeps
	 * the canvas in the system. It is recomputed only when the theme flips, never
	 * per frame — `getComputedStyle` forces a style flush and would be far too
	 * expensive inside the draw loop.
	 */
	const palette = $state({
		bg: '',
		node: '',
		nodeHover: '',
		nodeDim: '',
		edge: '',
		edgeHighlight: '',
		label: '',
		labelDim: ''
	});

	function resolvePalette() {
		if (!container) return;
		const cs = getComputedStyle(container);
		const token = (name: string, fallback: string) => cs.getPropertyValue(name).trim() || fallback;

		palette.bg = token('--color-bg-primary', '#0a0a0a');
		palette.node = token('--color-bg-brand', '#ff6633');
		palette.nodeHover = token('--color-text-brand', '#ff8250');
		palette.edge = token('--color-border-strong', 'rgba(128,128,128,0.35)');
		palette.edgeHighlight = token('--color-brand-32', 'rgba(255,102,51,0.5)');
		palette.nodeDim = token('--color-brand-32', 'rgba(255,102,51,0.3)');
		palette.label = token('--color-text-primary', '#dcddde');
		palette.labelDim = token('--color-text-tertiary', '#888');
	}

	$effect(() => {
		// Depend on the theme so a toggle re-resolves every colour.
		const _ = theme.current;
		resolvePalette();
	});

	// ─── Flat render state, kept in sync with the worker ──────────────────────
	// Positions are interleaved [x0,y0, x1,y1, ...] so a worker message can swap
	// the whole buffer in with no per-node copy.
	let count = 0;
	let pos = new Float32Array(0);
	let nodeIds: string[] = [];
	let nodeLabels: string[] = [];
	let nodeDeg = new Float32Array(0); // degree, for node sizing
	let edgePairs = new Int32Array(0); // [s,t,...] node-index pairs, for drawing
	// lowercased id -> index; built by buildFlatGraph(), only read here
	let nodeIndexLc: Map<string, number> = new Map();

	// Camera
	let camX = 0;
	let camY = 0;
	let camScale = 1;

	// Interaction (indices into the flat arrays; -1 = none)
	let dragIndex = -1;
	let dragWX = 0; // world position of the dragged node (re-pinned on each tick)
	let dragWY = 0;
	let hoverIndex = -1;
	let isPanning = false;
	let panStartX = 0;
	let panStartY = 0;
	let panCamStartX = 0;
	let panCamStartY = 0;
	let mouseDownX = 0;
	let mouseDownY = 0;
	let didDrag = false;

	/**
	 * Rebuild the flat render arrays from graph data and hand the new state to
	 * the worker. `seed` produces the initial position for each node — the only
	 * thing that differs between a fresh init (circle layout) and an incremental
	 * update (carry existing positions).
	 */
	function applyGraph(data: GraphInput, seed: SeedFn) {
		totalNodeCount = data.nodes.length;
		const flat = buildFlatGraph(data, MAX_SIM_NODES, seed);
		count = flat.count;
		nodeIds = flat.nodeIds;
		nodeLabels = flat.nodeLabels;
		nodeDeg = flat.nodeDeg;
		nodeIndexLc = flat.nodeIndexLc;
		pos = flat.pos;
		edgePairs = flat.edgePairs;

		sendInit();
	}

	/** Hand the current flat state to the worker (transfers copies of the buffers). */
	function sendInit() {
		if (!worker) return;
		const p = pos.slice();
		const e = edgePairs.slice();
		workerActive = count > 0;
		worker.postMessage({ type: 'init', count, positions: p.buffer, edges: e.buffer }, [
			p.buffer,
			e.buffer
		]);
		ensureLoop();
	}

	function initGraph(data: GraphInput) {
		applyGraph(data, (_id, i, total) => {
			const angle = (i / total) * Math.PI * 2;
			const radius = Math.sqrt(total) * 30;
			return [
				Math.cos(angle) * radius + (Math.random() - 0.5) * 20,
				Math.sin(angle) * radius + (Math.random() - 0.5) * 20
			];
		});
		// Center camera for a fresh layout.
		camX = 0;
		camY = 0;
		camScale = 1;
	}

	function updateGraph(data: GraphInput) {
		// Carry existing positions (by lowercased id) so the layout doesn't jump.
		const prev = snapshotPositions(count, nodeIds, pos);
		applyGraph(data, (id) => {
			const existing = prev.get(id.toLowerCase());
			if (existing) return existing;
			// New node — place near center with slight randomness.
			const angle = Math.random() * Math.PI * 2;
			const radius = 50 + Math.random() * 50;
			return [Math.cos(angle) * radius, Math.sin(angle) * radius];
		});
		// Keep the user's pan/zoom on incremental updates.
	}

	function onWorkerMessage(ev: MessageEvent) {
		const msg = ev.data as
			| { type: 'positions'; positions: ArrayBuffer; count: number }
			| { type: 'settled' };
		if (msg.type === 'positions') {
			// Ignore stale frames from a previous graph generation.
			if (msg.count !== count) return;
			const incoming = new Float32Array(msg.positions);
			pos = incoming;
			// Keep the node the user is actively dragging pinned to the cursor.
			if (dragIndex >= 0 && dragIndex < count) {
				pos[2 * dragIndex] = dragWX;
				pos[2 * dragIndex + 1] = dragWY;
			}
			requestDraw();
		} else if (msg.type === 'settled') {
			workerActive = false;
		}
	}

	function nodeRadius(deg: number): number {
		return Math.max(3, Math.min(10, 3 + deg * 1.5));
	}

	function stemOf(filePath: string): string {
		const name = filePath.split('/').pop() ?? '';
		return (name.endsWith('.md') ? name.slice(0, -3) : name).toLowerCase();
	}

	function draw() {
		if (!canvas || !container) return;
		const ctx = canvas.getContext('2d');
		if (!ctx) return;

		const w = container.clientWidth;
		const h = container.clientHeight;
		const dpr = window.devicePixelRatio || 1;

		if (canvas.width !== w * dpr || canvas.height !== h * dpr) {
			canvas.width = w * dpr;
			canvas.height = h * dpr;
			canvas.style.width = `${w}px`;
			canvas.style.height = `${h}px`;
		}

		ctx.setTransform(dpr, 0, 0, dpr, 0, 0);
		ctx.fillStyle = palette.bg;
		ctx.fillRect(0, 0, w, h);

		ctx.save();
		ctx.translate(w / 2 + camX, h / 2 + camY);
		ctx.scale(camScale, camScale);

		// Find the hovered node's connected edges/nodes.
		const { edges: highlightEdges, nodes: highlightNodes } = computeHighlights(
			edgePairs,
			hoverIndex
		);

		// Active file node (white)
		let activeIndex = -1;
		if (files.activeFile) {
			const idx = nodeIndexLc.get(stemOf(files.activeFile));
			if (idx !== undefined) activeIndex = idx;
		}

		// Draw edges. Canvas state changes (strokeStyle/lineWidth) and stroke()
		// calls are expensive, so batch edges that share styling into a single
		// path. The common case — nothing hovered — collapses to one stroke().
		if (hoverIndex < 0) {
			ctx.strokeStyle = palette.edge;
			ctx.lineWidth = 0.5;
			ctx.beginPath();
			for (let i = 0; i < edgePairs.length; i += 2) {
				const s = edgePairs[i];
				const t = edgePairs[i + 1];
				ctx.moveTo(pos[2 * s], pos[2 * s + 1]);
				ctx.lineTo(pos[2 * t], pos[2 * t + 1]);
			}
			ctx.stroke();
		} else {
			// Two passes: normal edges, then highlighted edges over the top.
			ctx.strokeStyle = palette.edge;
			ctx.lineWidth = 0.5;
			ctx.beginPath();
			for (let i = 0; i < edgePairs.length; i += 2) {
				if (highlightEdges.has(i)) continue;
				const s = edgePairs[i];
				const t = edgePairs[i + 1];
				ctx.moveTo(pos[2 * s], pos[2 * s + 1]);
				ctx.lineTo(pos[2 * t], pos[2 * t + 1]);
			}
			ctx.stroke();

			ctx.strokeStyle = palette.edgeHighlight;
			ctx.lineWidth = 1.5;
			ctx.beginPath();
			for (const i of highlightEdges) {
				const s = edgePairs[i];
				const t = edgePairs[i + 1];
				ctx.moveTo(pos[2 * s], pos[2 * s + 1]);
				ctx.lineTo(pos[2 * t], pos[2 * t + 1]);
			}
			ctx.stroke();
		}

		// Draw nodes. Same batching idea: when nothing is hovered every node is the
		// same color (except the active note), so they collapse into one fill().
		if (hoverIndex < 0) {
			ctx.fillStyle = palette.node;
			ctx.beginPath();
			for (let i = 0; i < count; i++) {
				if (i === activeIndex) continue;
				const r = nodeRadius(nodeDeg[i]);
				const x = pos[2 * i];
				const y = pos[2 * i + 1];
				ctx.moveTo(x + r, y);
				ctx.arc(x, y, r, 0, Math.PI * 2);
			}
			ctx.fill();

			if (activeIndex >= 0) {
				const r = nodeRadius(nodeDeg[activeIndex]);
				// The open note is drawn in the highest-contrast colour available, so
				// it stands out from the brand-coloured field of every other node.
				ctx.fillStyle = palette.label;
				ctx.beginPath();
				ctx.arc(pos[2 * activeIndex], pos[2 * activeIndex + 1], r, 0, Math.PI * 2);
				ctx.fill();
			}
		} else {
			// Hover path: per-node colors (dim unconnected, highlight the rest).
			for (let i = 0; i < count; i++) {
				const r = nodeRadius(nodeDeg[i]);
				const isHovered = i === hoverIndex;
				const isHighlighted = highlightNodes.has(i);
				const isActive = i === activeIndex;

				ctx.beginPath();
				ctx.arc(pos[2 * i], pos[2 * i + 1], r, 0, Math.PI * 2);

				if (isActive) {
					ctx.fillStyle = palette.label;
				} else if (isHovered) {
					ctx.fillStyle = palette.nodeHover;
				} else if (isHighlighted) {
					ctx.fillStyle = palette.node;
				} else {
					ctx.fillStyle = palette.nodeDim;
				}
				ctx.fill();
			}
		}

		// Draw labels (only when zoomed in enough or for hovered/connected)
		const showAllLabels = camScale >= 1.2;
		ctx.font = `${11 / camScale > 14 ? 14 : Math.max(9, 11 / camScale)}px Inter, sans-serif`;
		ctx.textAlign = 'center';
		ctx.textBaseline = 'top';

		for (let i = 0; i < count; i++) {
			const isHighlighted = hoverIndex >= 0 ? highlightNodes.has(i) : false;
			if (!showAllLabels && !isHighlighted) continue;
			const r = nodeRadius(nodeDeg[i]);
			ctx.fillStyle = isHighlighted ? palette.label : palette.labelDim;
			ctx.fillText(nodeLabels[i], pos[2 * i], pos[2 * i + 1] + r + 3);
		}

		ctx.restore();
	}

	function screenToWorld(sx: number, sy: number): { wx: number; wy: number } {
		if (!container) return { wx: 0, wy: 0 };
		const w = container.clientWidth;
		const h = container.clientHeight;
		const wx = (sx - w / 2 - camX) / camScale;
		const wy = (sy - h / 2 - camY) / camScale;
		return { wx, wy };
	}

	/** Index of the top-most node under a screen point, or -1. */
	function findNodeAt(sx: number, sy: number): number {
		const { wx, wy } = screenToWorld(sx, sy);
		for (let i = count - 1; i >= 0; i--) {
			const r = nodeRadius(nodeDeg[i]) + 4; // hit slop
			const dx = pos[2 * i] - wx;
			const dy = pos[2 * i + 1] - wy;
			if (dx * dx + dy * dy <= r * r) return i;
		}
		return -1;
	}

	function handleMouseDown(e: MouseEvent) {
		const rect = canvas?.getBoundingClientRect();
		if (!rect) return;
		const sx = e.clientX - rect.left;
		const sy = e.clientY - rect.top;
		mouseDownX = e.clientX;
		mouseDownY = e.clientY;
		didDrag = false;

		const idx = findNodeAt(sx, sy);
		if (idx >= 0) {
			dragIndex = idx;
			dragWX = pos[2 * idx];
			dragWY = pos[2 * idx + 1];
			worker?.postMessage({ type: 'drag', index: idx, x: dragWX, y: dragWY });
			ensureLoop();
		} else {
			isPanning = true;
			panStartX = e.clientX;
			panStartY = e.clientY;
			panCamStartX = camX;
			panCamStartY = camY;
		}
	}

	function handleMouseMove(e: MouseEvent) {
		const rect = canvas?.getBoundingClientRect();
		if (!rect) return;
		const sx = e.clientX - rect.left;
		const sy = e.clientY - rect.top;

		if (dragIndex >= 0) {
			const dx = e.clientX - mouseDownX;
			const dy = e.clientY - mouseDownY;
			if (dx * dx + dy * dy > 16) didDrag = true;
			const { wx, wy } = screenToWorld(sx, sy);
			dragWX = wx;
			dragWY = wy;
			pos[2 * dragIndex] = wx;
			pos[2 * dragIndex + 1] = wy;
			worker?.postMessage({ type: 'drag', index: dragIndex, x: wx, y: wy });
			requestDraw();
		} else if (isPanning) {
			camX = panCamStartX + (e.clientX - panStartX);
			camY = panCamStartY + (e.clientY - panStartY);
			requestDraw();
		} else {
			const prev = hoverIndex;
			hoverIndex = findNodeAt(sx, sy);
			if (canvas) {
				canvas.style.cursor = hoverIndex >= 0 ? 'pointer' : 'grab';
			}
			if (prev !== hoverIndex) requestDraw();
		}
	}

	function handleMouseUp() {
		if (dragIndex >= 0) {
			dragIndex = -1;
			worker?.postMessage({ type: 'dragEnd' });
		}
		isPanning = false;
	}

	function handleClick(e: MouseEvent) {
		if (didDrag) return;
		const rect = canvas?.getBoundingClientRect();
		if (!rect) return;
		const sx = e.clientX - rect.left;
		const sy = e.clientY - rect.top;

		const idx = findNodeAt(sx, sy);
		if (idx >= 0) {
			const match = graph.nodeToPath.get(nodeIds[idx].toLowerCase());
			if (match) {
				onfileselect(match);
			}
		}
	}

	function handleWheel(e: WheelEvent) {
		e.preventDefault();
		const factor = e.deltaY > 0 ? 0.9 : 1.1;
		const newScale = Math.max(0.1, Math.min(5, camScale * factor));

		// Zoom toward mouse position
		if (container) {
			const rect = container.getBoundingClientRect();
			const mx = e.clientX - rect.left;
			const my = e.clientY - rect.top;
			const cx = container.clientWidth / 2 + camX;
			const cy = container.clientHeight / 2 + camY;
			const dx = mx - cx;
			const dy = my - cy;
			camX += dx * (1 - newScale / camScale);
			camY += dy * (1 - newScale / camScale);
		}

		camScale = newScale;
		requestDraw();
	}

	// ─── Render loop ──────────────────────────────────────────────────────────
	// The loop keeps running while the worker is laying out or a drag is active;
	// otherwise a single frame is drawn on demand (camera/hover changes) and the
	// loop parks itself, mirroring the previous sleep/wake behavior.
	function loop() {
		rafId = 0;
		if (!mounted) return;
		draw();
		if (workerActive || dragIndex >= 0) {
			rafId = requestAnimationFrame(loop);
		}
	}

	function ensureLoop() {
		if (!rafId && mounted) rafId = requestAnimationFrame(loop);
	}

	function requestDraw() {
		ensureLoop();
	}

	async function refresh() {
		await graph.build();
		initGraph(graph.data);
	}

	let prevNodeCount = 0;

	$effect(() => {
		// Re-init when graph data changes
		const nodeCount = graph.data.nodes.length;
		if (nodeCount > 0 && mounted) {
			const changeRatio =
				prevNodeCount > 0 ? Math.abs(nodeCount - prevNodeCount) / prevNodeCount : 1;

			if (changeRatio > 0.2 || prevNodeCount === 0) {
				// Large change or first load — full re-init
				initGraph(graph.data);
			} else {
				// Small change — update data but preserve positions
				updateGraph(graph.data);
			}
			prevNodeCount = nodeCount;
		}
	});

	onMount(async () => {
		mounted = true;
		worker = new Worker(new URL('../graph/layout.worker.ts', import.meta.url), {
			type: 'module'
		});
		worker.onmessage = onWorkerMessage;
		// Resolve the palette once the container is in the DOM and the tokens are
		// actually computable.
		resolvePalette();
		await refresh();
	});

	onDestroy(() => {
		mounted = false;
		if (rafId) cancelAnimationFrame(rafId);
		worker?.terminate();
		worker = null;
	});
</script>

<!-- `rounded-xs!` / `p-0!`: app.css's base `button` rule is unlayered, so it
     outranks everything Tailwind emits into `@layer utilities`; radius and
     padding on a <button> only stick when marked important. -->
<div class="relative flex h-full w-full flex-col overflow-hidden" bind:this={container}>
	<div class="flex shrink-0 items-center gap-2 px-2.5 py-1.5">
		<button
			class="flex size-6 cursor-pointer items-center justify-center rounded-xs p-0 text-subtle-foreground transition-colors enabled:hover:bg-surface-3 enabled:hover:text-foreground"
			onclick={refresh}
			title="Refresh graph"
			disabled={graph.loading}
		>
			<RefreshCw size={14} class={graph.loading ? 'animate-spin' : ''} />
		</button>
		<span class="text-xs text-subtle-foreground">
			{graph.data.nodes.length} notes · {graph.data.edges.length} links
			{#if totalNodeCount > MAX_SIM_NODES}
				<span title="Showing top {MAX_SIM_NODES} most-connected notes out of {totalNodeCount}">
					(top {MAX_SIM_NODES} of {totalNodeCount})
				</span>
			{/if}
		</span>
	</div>
	<canvas
		class="block w-full flex-1"
		bind:this={canvas}
		onmousedown={handleMouseDown}
		onmousemove={handleMouseMove}
		onmouseup={handleMouseUp}
		onmouseleave={handleMouseUp}
		onclick={handleClick}
		onwheel={handleWheel}
		style="cursor: grab"
	></canvas>
</div>
