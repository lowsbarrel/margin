<script lang="ts">
  import { onMount, onDestroy } from "svelte";
  import {
    graph,
    type GraphNode,
    type GraphEdge,
  } from "$lib/stores/graph.svelte";
  import { files } from "$lib/stores/files.svelte";
  import { theme } from "$lib/stores/theme.svelte";
  import { RefreshCw } from "lucide-svelte";

  interface Props {
    onfileselect: (path: string) => void;
  }

  let { onfileselect }: Props = $props();

  let canvas = $state<HTMLCanvasElement | null>(null);
  let container = $state<HTMLDivElement | null>(null);
  let animFrameId = 0;
  let mounted = false;
  let simSleeping = false;
  const SLEEP_VELOCITY_SQ = 0.01; // sleep when all nodes slower than ~0.1px/frame

  // Cap the simulation at this many nodes. On vaults with more notes,
  // only the most-connected nodes are simulated (the rest appear as isolated
  // nodes if they have no edges). This keeps the O(n²) repulsion loop bounded.
  const MAX_SIM_NODES = 500;
  let totalNodeCount = $state(0); // full count before cap, for UI badge

  // Cached background colour — recomputed only on theme change, not per frame
  let cachedBgColor = "#0a0a0a";
  $effect(() => {
    const _ = theme.current;
    if (container) {
      cachedBgColor =
        getComputedStyle(container).getPropertyValue("--bg-primary").trim() ||
        "#0a0a0a";
    }
  });

  // Simulation state
  interface SimNode {
    id: string;
    label: string;
    x: number;
    y: number;
    vx: number;
    vy: number;
    edges: number; // connection count for sizing
  }

  let simNodes: SimNode[] = [];
  let simEdges: { source: number; target: number }[] = [];

  // Camera
  let camX = $state(0);
  let camY = $state(0);
  let camScale = $state(1);

  // Interaction
  let dragNode: SimNode | null = null;
  let isPanning = false;
  let panStartX = 0;
  let panStartY = 0;
  let panCamStartX = 0;
  let panCamStartY = 0;
  let hoveredNode: SimNode | null = null;
  let mouseDownX = 0;
  let mouseDownY = 0;
  let didDrag = false;

  // Colors
  const NODE_COLOR = "rgba(255, 102, 51, 0.9)";
  const NODE_HOVER_COLOR = "rgba(255, 130, 80, 1)";
  const EDGE_COLOR = "rgba(100, 100, 100, 0.35)";
  const EDGE_HIGHLIGHT_COLOR = "rgba(255, 102, 51, 0.5)";
  const LABEL_COLOR = "rgba(220, 221, 222, 0.85)";

  function initSimulation(data: { nodes: GraphNode[]; edges: GraphEdge[] }) {
    totalNodeCount = data.nodes.length;

    // Cap nodes: keep the most-connected ones so the O(n²) repulsion stays bounded.
    let cappedNodes = data.nodes;
    let cappedEdges = data.edges;
    if (data.nodes.length > MAX_SIM_NODES) {
      const edgeCount = new Map<string, number>();
      for (const e of data.edges) {
        edgeCount.set(e.source, (edgeCount.get(e.source) ?? 0) + 1);
        edgeCount.set(e.target, (edgeCount.get(e.target) ?? 0) + 1);
      }
      cappedNodes = [...data.nodes]
        .sort((a, b) => (edgeCount.get(b.id) ?? 0) - (edgeCount.get(a.id) ?? 0))
        .slice(0, MAX_SIM_NODES);
      const cappedIds = new Set(cappedNodes.map((n) => n.id));
      cappedEdges = data.edges.filter(
        (e) => cappedIds.has(e.source) && cappedIds.has(e.target),
      );
    }

    const nodeIndex = new Map<string, number>();

    // Count edges per node (within capped set)
    const edgeCounts = new Map<string, number>();
    for (const e of cappedEdges) {
      edgeCounts.set(e.source, (edgeCounts.get(e.source) ?? 0) + 1);
      edgeCounts.set(e.target, (edgeCounts.get(e.target) ?? 0) + 1);
    }

    simNodes = cappedNodes.map((n, i) => {
      nodeIndex.set(n.id, i);
      const angle = (i / cappedNodes.length) * Math.PI * 2;
      const radius = Math.sqrt(cappedNodes.length) * 30;
      return {
        id: n.id,
        label: n.label,
        x: Math.cos(angle) * radius + (Math.random() - 0.5) * 20,
        y: Math.sin(angle) * radius + (Math.random() - 0.5) * 20,
        vx: 0,
        vy: 0,
        edges: edgeCounts.get(n.id) ?? 0,
      };
    });

    simEdges = [];
    for (const e of cappedEdges) {
      const si = nodeIndex.get(e.source);
      const ti = nodeIndex.get(e.target);
      if (si !== undefined && ti !== undefined && si !== ti) {
        simEdges.push({ source: si, target: ti });
      }
    }

    // Center camera
    camX = 0;
    camY = 0;
    camScale = 1;
    // Wake the simulation for the new data
    wakeSimulation();
  }

  /** Returns the maximum velocity² across all non-dragged nodes this tick. */
  function simulate(): number {
    const alpha = 0.3;
    const repulsion = 800;
    const attraction = 0.005;
    const idealLength = 100;
    const damping = 0.85;
    const centerPull = 0.01;

    // ─── Barnes-Hut repulsion (O(n log n)) ───────────────────────
    // Build a quadtree, then for each node approximate distant groups.
    const THETA = 0.8; // Barnes-Hut opening angle threshold

    interface QTNode {
      cx: number; cy: number; mass: number;
      x1: number; y1: number; x2: number; y2: number;
      children: (QTNode | null)[];
      body: SimNode | null;
    }

    function qtNew(x1: number, y1: number, x2: number, y2: number): QTNode {
      return { cx: 0, cy: 0, mass: 0, x1, y1, x2, y2, children: [null, null, null, null], body: null };
    }

    function qtInsert(qt: QTNode, n: SimNode) {
      if (qt.mass === 0) {
        qt.body = n;
        qt.cx = n.x;
        qt.cy = n.y;
        qt.mass = 1;
        return;
      }
      if (qt.body) {
        // Subdivide — push existing body down
        const existing = qt.body;
        qt.body = null;
        qtPush(qt, existing);
      }
      // Update center of mass
      qt.cx = (qt.cx * qt.mass + n.x) / (qt.mass + 1);
      qt.cy = (qt.cy * qt.mass + n.y) / (qt.mass + 1);
      qt.mass += 1;
      qtPush(qt, n);
    }

    function qtPush(qt: QTNode, n: SimNode) {
      const mx = (qt.x1 + qt.x2) / 2;
      const my = (qt.y1 + qt.y2) / 2;
      const idx = (n.x > mx ? 1 : 0) + (n.y > my ? 2 : 0);
      if (!qt.children[idx]) {
        const x1 = idx & 1 ? mx : qt.x1;
        const y1 = idx & 2 ? my : qt.y1;
        const x2 = idx & 1 ? qt.x2 : mx;
        const y2 = idx & 2 ? qt.y2 : my;
        qt.children[idx] = qtNew(x1, y1, x2, y2);
      }
      qtInsert(qt.children[idx]!, n);
    }

    function qtApplyForce(qt: QTNode, n: SimNode, alpha: number, repulsion: number) {
      if (qt.mass === 0) return;
      let dx = qt.cx - n.x;
      let dy = qt.cy - n.y;
      const distSq = dx * dx + dy * dy || 1;
      const size = qt.x2 - qt.x1;

      // If leaf with a single body, apply direct force (skip self)
      if (qt.body) {
        if (qt.body === n) return;
        const dist = Math.sqrt(distSq);
        const force = repulsion / distSq;
        const fx = (dx / dist) * force;
        const fy = (dy / dist) * force;
        n.vx -= fx * alpha;
        n.vy -= fy * alpha;
        return;
      }

      // Barnes-Hut criterion: if cell is far enough, use center-of-mass approx
      if (size * size / distSq < THETA * THETA) {
        const dist = Math.sqrt(distSq);
        const force = (repulsion * qt.mass) / distSq;
        const fx = (dx / dist) * force;
        const fy = (dy / dist) * force;
        n.vx -= fx * alpha;
        n.vy -= fy * alpha;
        return;
      }

      // Otherwise recurse into children
      for (const child of qt.children) {
        if (child) qtApplyForce(child, n, alpha, repulsion);
      }
    }

    // Determine bounding box
    let bx1 = Infinity, by1 = Infinity, bx2 = -Infinity, by2 = -Infinity;
    for (const n of simNodes) {
      if (n.x < bx1) bx1 = n.x;
      if (n.y < by1) by1 = n.y;
      if (n.x > bx2) bx2 = n.x;
      if (n.y > by2) by2 = n.y;
    }
    // Pad to avoid zero-size
    const pad = Math.max(1, (bx2 - bx1) * 0.01, (by2 - by1) * 0.01);
    const root = qtNew(bx1 - pad, by1 - pad, bx2 + pad, by2 + pad);
    for (const n of simNodes) qtInsert(root, n);
    for (const n of simNodes) qtApplyForce(root, n, alpha, repulsion);

    // Attraction along edges
    for (const e of simEdges) {
      const a = simNodes[e.source];
      const b = simNodes[e.target];
      let dx = b.x - a.x;
      let dy = b.y - a.y;
      let dist = Math.sqrt(dx * dx + dy * dy) || 1;
      const force = (dist - idealLength) * attraction;
      const fx = (dx / dist) * force;
      const fy = (dy / dist) * force;
      a.vx += fx * alpha;
      a.vy += fy * alpha;
      b.vx -= fx * alpha;
      b.vy -= fy * alpha;
    }

    // Center gravity
    for (const n of simNodes) {
      n.vx -= n.x * centerPull * alpha;
      n.vy -= n.y * centerPull * alpha;
    }

    // Apply velocity and track max kinetic energy
    let maxV2 = 0;
    for (const n of simNodes) {
      if (n === dragNode) continue;
      n.vx *= damping;
      n.vy *= damping;
      n.x += n.vx;
      n.y += n.vy;
      const v2 = n.vx * n.vx + n.vy * n.vy;
      if (v2 > maxV2) maxV2 = v2;
    }
    return maxV2;
  }

  function nodeRadius(n: SimNode): number {
    return Math.max(3, Math.min(10, 3 + n.edges * 1.5));
  }

  function draw() {
    if (!canvas || !container) return;
    const ctx = canvas.getContext("2d");
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
    ctx.fillStyle = cachedBgColor;
    ctx.fillRect(0, 0, w, h);

    ctx.save();
    ctx.translate(w / 2 + camX, h / 2 + camY);
    ctx.scale(camScale, camScale);

    // Find hovered node's connected edges
    const highlightEdges = new Set<number>();
    const highlightNodes = new Set<string>();
    if (hoveredNode) {
      highlightNodes.add(hoveredNode.id);
      for (let i = 0; i < simEdges.length; i++) {
        const e = simEdges[i];
        if (
          simNodes[e.source] === hoveredNode ||
          simNodes[e.target] === hoveredNode
        ) {
          highlightEdges.add(i);
          highlightNodes.add(simNodes[e.source].id);
          highlightNodes.add(simNodes[e.target].id);
        }
      }
    }

    // Draw edges
    for (let i = 0; i < simEdges.length; i++) {
      const e = simEdges[i];
      const a = simNodes[e.source];
      const b = simNodes[e.target];
      ctx.beginPath();
      ctx.moveTo(a.x, a.y);
      ctx.lineTo(b.x, b.y);
      ctx.strokeStyle = highlightEdges.has(i)
        ? EDGE_HIGHLIGHT_COLOR
        : EDGE_COLOR;
      ctx.lineWidth = highlightEdges.has(i) ? 1.5 : 0.5;
      ctx.stroke();
    }

    // Draw nodes
    for (const n of simNodes) {
      const r = nodeRadius(n);
      const isHighlighted = hoveredNode ? highlightNodes.has(n.id) : false;
      const isHovered = n === hoveredNode;
      const isActive =
        files.activeFile && fileMatchesNode(files.activeFile, n.id);

      ctx.beginPath();
      ctx.arc(n.x, n.y, r, 0, Math.PI * 2);

      if (isActive) {
        ctx.fillStyle = "#fff";
      } else if (isHovered) {
        ctx.fillStyle = NODE_HOVER_COLOR;
      } else if (isHighlighted) {
        ctx.fillStyle = NODE_COLOR;
      } else if (hoveredNode) {
        ctx.fillStyle = "rgba(255, 102, 51, 0.3)";
      } else {
        ctx.fillStyle = NODE_COLOR;
      }
      ctx.fill();
    }

    // Draw labels (only when zoomed in enough or for hovered/connected)
    const showAllLabels = camScale >= 1.2;
    ctx.font = `${11 / camScale > 14 ? 14 : Math.max(9, 11 / camScale)}px Inter, sans-serif`;
    ctx.textAlign = "center";
    ctx.textBaseline = "top";

    for (const n of simNodes) {
      const isHighlighted = hoveredNode ? highlightNodes.has(n.id) : false;
      if (!showAllLabels && !isHighlighted) continue;

      const r = nodeRadius(n);
      ctx.fillStyle = isHighlighted ? LABEL_COLOR : "rgba(220, 221, 222, 0.5)";
      ctx.fillText(n.label, n.x, n.y + r + 3);
    }

    ctx.restore();
  }

  function fileMatchesNode(filePath: string, nodeId: string): boolean {
    const name = filePath.split("/").pop() ?? "";
    const stem = name.endsWith(".md") ? name.slice(0, -3) : name;
    return stem.toLowerCase() === nodeId.toLowerCase();
  }

  function screenToWorld(sx: number, sy: number): { wx: number; wy: number } {
    if (!container) return { wx: 0, wy: 0 };
    const w = container.clientWidth;
    const h = container.clientHeight;
    const wx = (sx - w / 2 - camX) / camScale;
    const wy = (sy - h / 2 - camY) / camScale;
    return { wx, wy };
  }

  function findNodeAt(sx: number, sy: number): SimNode | null {
    const { wx, wy } = screenToWorld(sx, sy);
    // Search in reverse for top-most drawn node
    for (let i = simNodes.length - 1; i >= 0; i--) {
      const n = simNodes[i];
      const r = nodeRadius(n) + 4; // hit slop
      const dx = n.x - wx;
      const dy = n.y - wy;
      if (dx * dx + dy * dy <= r * r) return n;
    }
    return null;
  }

  function handleMouseDown(e: MouseEvent) {
    wakeSimulation();
    const rect = canvas?.getBoundingClientRect();
    if (!rect) return;
    const sx = e.clientX - rect.left;
    const sy = e.clientY - rect.top;
    mouseDownX = e.clientX;
    mouseDownY = e.clientY;
    didDrag = false;

    const node = findNodeAt(sx, sy);
    if (node) {
      dragNode = node;
      dragNode.vx = 0;
      dragNode.vy = 0;
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

    if (dragNode) {
      const dx = e.clientX - mouseDownX;
      const dy = e.clientY - mouseDownY;
      if (dx * dx + dy * dy > 16) didDrag = true;
      const { wx, wy } = screenToWorld(sx, sy);
      dragNode.x = wx;
      dragNode.y = wy;
      dragNode.vx = 0;
      dragNode.vy = 0;
    } else if (isPanning) {
      camX = panCamStartX + (e.clientX - panStartX);
      camY = panCamStartY + (e.clientY - panStartY);
    } else {
      const prevHovered = hoveredNode;
      hoveredNode = findNodeAt(sx, sy);
      if (canvas) {
        canvas.style.cursor = hoveredNode ? "pointer" : "grab";
      }
      // If hover state changed, ensure the canvas is redrawn
      if (prevHovered !== hoveredNode) {
        if (hoveredNode) {
          wakeSimulation();
        } else {
          // Hover cleared — draw one final frame to remove highlighting
          draw();
        }
      }
    }
  }

  function handleMouseUp() {
    if (dragNode) {
      dragNode = null;
    }
    isPanning = false;
  }

  function handleClick(e: MouseEvent) {
    if (didDrag) return;
    const rect = canvas?.getBoundingClientRect();
    if (!rect) return;
    const sx = e.clientX - rect.left;
    const sy = e.clientY - rect.top;

    const node = findNodeAt(sx, sy);
    if (node) {
      const match = graph.nodeToPath.get(node.id.toLowerCase());
      if (match) {
        onfileselect(match);
      }
    }
  }

  function handleWheel(e: WheelEvent) {
    wakeSimulation();
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
  }

  function loop() {
    if (!mounted) return;
    const maxV2 = simulate();
    draw();
    // Stop the loop when nodes have settled (no drag or hover active)
    if (maxV2 <= SLEEP_VELOCITY_SQ && !dragNode && !hoveredNode) {
      simSleeping = true;
      animFrameId = 0;
    } else {
      animFrameId = requestAnimationFrame(loop);
    }
  }

  function wakeSimulation() {
    if (!simSleeping || !mounted) return;
    simSleeping = false;
    loop();
  }

  async function refresh() {
    await graph.build();
    initSimulation(graph.data);
  }

  $effect(() => {
    // Re-init when graph data changes
    if (graph.data.nodes.length > 0 && mounted) {
      initSimulation(graph.data);
    }
  });

  onMount(async () => {
    mounted = true;
    // Seed the cached bg color once the container is in the DOM
    if (container) {
      cachedBgColor =
        getComputedStyle(container).getPropertyValue("--bg-primary").trim() ||
        "#0a0a0a";
    }
    await refresh();
    loop();
  });

  onDestroy(() => {
    mounted = false;
    if (animFrameId) cancelAnimationFrame(animFrameId);
  });
</script>

<div class="graph-view" bind:this={container}>
  <div class="graph-toolbar">
    <button
      class="graph-refresh"
      onclick={refresh}
      title="Refresh graph"
      disabled={graph.loading}
    >
      <RefreshCw size={14} class={graph.loading ? "spinning" : ""} />
    </button>
    <span class="graph-stats">
      {graph.data.nodes.length} notes · {graph.data.edges.length} links
      {#if totalNodeCount > MAX_SIM_NODES}
        <span
          class="graph-cap-badge"
          title="Showing top {MAX_SIM_NODES} most-connected notes out of {totalNodeCount}"
        >
          (top {MAX_SIM_NODES} of {totalNodeCount})
        </span>
      {/if}
    </span>
  </div>
  <canvas
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

<style>
  .graph-view {
    display: flex;
    flex-direction: column;
    width: 100%;
    height: 100%;
    position: relative;
    overflow: hidden;
  }

  .graph-toolbar {
    display: flex;
    align-items: center;
    gap: 8px;
    padding: 6px 10px;
    flex-shrink: 0;
  }

  .graph-refresh {
    display: flex;
    align-items: center;
    justify-content: center;
    width: 24px;
    height: 24px;
    padding: 0;
    background: transparent;
    border: none;
    border-radius: var(--radius-xs);
    color: var(--text-muted);
    cursor: pointer;
    transition:
      color 0.12s,
      background 0.12s;
  }

  .graph-refresh:hover:not(:disabled) {
    color: var(--text-primary);
    background: var(--bg-hover);
  }

  .graph-refresh:disabled {
    opacity: 0.4;
    cursor: not-allowed;
  }

  .graph-refresh :global(.spinning) {
    animation: spin 1s linear infinite;
  }

  @keyframes spin {
    from {
      transform: rotate(0deg);
    }
    to {
      transform: rotate(360deg);
    }
  }

  .graph-stats {
    font-size: 0.7rem;
    color: var(--text-muted);
  }

  canvas {
    flex: 1;
    width: 100%;
    display: block;
  }
</style>
