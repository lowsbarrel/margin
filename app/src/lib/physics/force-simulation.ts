/**
 * force-simulation.ts
 *
 * Extracted from GraphView.svelte – Barnes-Hut O(n log n) force-directed
 * graph simulation.
 *
 * CanvasEditor.svelte has NO simulation logic; its camera model is also
 * structurally different from GraphView's (see coordinate-transforms note at
 * the bottom of this file), so coordinate helpers are NOT shared here.
 *
 * Usage in GraphView.svelte (once you wire it up):
 *
 *   import {
 *     type SimNode,
 *     type SimEdge,
 *     type SimulationOptions,
 *     simulate,
 *   } from "$lib/physics/force-simulation";
 */

// ---------------------------------------------------------------------------
// Types
// ---------------------------------------------------------------------------

export interface SimNode {
  id: string;
  label: string;
  x: number;
  y: number;
  vx: number;
  vy: number;
  /** Connection count, used for visual sizing outside the simulation. */
  edges: number;
}

export interface SimEdge {
  source: number; // index into SimNode[]
  target: number; // index into SimNode[]
}

export interface SimulationOptions {
  /** Simulation step size / global force scale. Default: 0.3 */
  alpha?: number;
  /** Node-node repulsion strength. Default: 800 */
  repulsion?: number;
  /** Spring constant along edges. Default: 0.005 */
  attraction?: number;
  /** Rest length of each spring edge (px). Default: 100 */
  idealLength?: number;
  /** Velocity damping per tick (0–1). Default: 0.85 */
  damping?: number;
  /** Gravity pulling all nodes toward the origin. Default: 0.01 */
  centerPull?: number;
  /** Barnes-Hut opening angle. Lower = more accurate, slower. Default: 0.8 */
  theta?: number;
}

// ---------------------------------------------------------------------------
// Internal: Barnes-Hut quadtree
// ---------------------------------------------------------------------------

interface QTNode {
  cx: number;
  cy: number;
  mass: number;
  x1: number;
  y1: number;
  x2: number;
  y2: number;
  children: [QTNode | null, QTNode | null, QTNode | null, QTNode | null];
  body: SimNode | null;
}

function qtNew(x1: number, y1: number, x2: number, y2: number): QTNode {
  return {
    cx: 0,
    cy: 0,
    mass: 0,
    x1,
    y1,
    x2,
    y2,
    children: [null, null, null, null],
    body: null,
  };
}

function qtInsert(qt: QTNode, n: SimNode): void {
  if (qt.mass === 0) {
    qt.body = n;
    qt.cx = n.x;
    qt.cy = n.y;
    qt.mass = 1;
    return;
  }
  if (qt.body) {
    // Subdivide – push existing leaf body down first.
    const existing = qt.body;
    qt.body = null;
    qtPush(qt, existing);
  }
  // Update centre-of-mass incrementally.
  qt.cx = (qt.cx * qt.mass + n.x) / (qt.mass + 1);
  qt.cy = (qt.cy * qt.mass + n.y) / (qt.mass + 1);
  qt.mass += 1;
  qtPush(qt, n);
}

function qtPush(qt: QTNode, n: SimNode): void {
  const mx = (qt.x1 + qt.x2) / 2;
  const my = (qt.y1 + qt.y2) / 2;
  // Quadrant index: bit 0 = right, bit 1 = bottom.
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

function qtApplyForce(
  qt: QTNode,
  n: SimNode,
  alpha: number,
  repulsion: number,
  theta: number,
): void {
  if (qt.mass === 0) return;
  const dx = qt.cx - n.x;
  const dy = qt.cy - n.y;
  const distSq = dx * dx + dy * dy || 1;
  const size = qt.x2 - qt.x1;

  if (qt.body) {
    // Leaf with a single body – direct force, skip self-interaction.
    if (qt.body === n) return;
    const dist = Math.sqrt(distSq);
    const force = repulsion / distSq;
    n.vx -= (dx / dist) * force * alpha;
    n.vy -= (dy / dist) * force * alpha;
    return;
  }

  // Barnes-Hut criterion: cell is far enough to use centre-of-mass.
  if ((size * size) / distSq < theta * theta) {
    const dist = Math.sqrt(distSq);
    const force = (repulsion * qt.mass) / distSq;
    n.vx -= (dx / dist) * force * alpha;
    n.vy -= (dy / dist) * force * alpha;
    return;
  }

  // Otherwise recurse into children.
  for (const child of qt.children) {
    if (child) qtApplyForce(child, n, alpha, repulsion, theta);
  }
}

// ---------------------------------------------------------------------------
// Public API
// ---------------------------------------------------------------------------

/**
 * Run one tick of the force simulation, mutating node positions and
 * velocities in-place.
 *
 * @param nodes    Array of simulation nodes (mutated in-place).
 * @param edges    Array of index-pairs connecting nodes.
 * @param dragNode The node currently being dragged by the user (skipped when
 *                 applying velocity); pass `null` when nothing is dragged.
 * @param options  Optional physics tuning parameters.
 * @returns        The maximum velocity² across all non-dragged nodes.  When
 *                 this falls below a threshold (e.g. 0.01) the simulation can
 *                 sleep.
 */
export function simulate(
  nodes: SimNode[],
  edges: SimEdge[],
  dragNode: SimNode | null,
  options: SimulationOptions = {},
): number {
  const alpha = options.alpha ?? 0.3;
  const repulsion = options.repulsion ?? 800;
  const attraction = options.attraction ?? 0.005;
  const idealLength = options.idealLength ?? 100;
  const damping = options.damping ?? 0.85;
  const centerPull = options.centerPull ?? 0.01;
  const theta = options.theta ?? 0.8;

  // ── Barnes-Hut repulsion (O(n log n)) ─────────────────────────────────────
  let bx1 = Infinity,
    by1 = Infinity,
    bx2 = -Infinity,
    by2 = -Infinity;
  for (const n of nodes) {
    if (n.x < bx1) bx1 = n.x;
    if (n.y < by1) by1 = n.y;
    if (n.x > bx2) bx2 = n.x;
    if (n.y > by2) by2 = n.y;
  }
  // Pad to avoid zero-size cells.
  const pad = Math.max(1, (bx2 - bx1) * 0.01, (by2 - by1) * 0.01);
  const root = qtNew(bx1 - pad, by1 - pad, bx2 + pad, by2 + pad);
  for (const n of nodes) qtInsert(root, n);
  for (const n of nodes) qtApplyForce(root, n, alpha, repulsion, theta);

  // ── Attraction along edges ─────────────────────────────────────────────────
  for (const e of edges) {
    const a = nodes[e.source];
    const b = nodes[e.target];
    const dx = b.x - a.x;
    const dy = b.y - a.y;
    const dist = Math.sqrt(dx * dx + dy * dy) || 1;
    const force = (dist - idealLength) * attraction;
    const fx = (dx / dist) * force;
    const fy = (dy / dist) * force;
    a.vx += fx * alpha;
    a.vy += fy * alpha;
    b.vx -= fx * alpha;
    b.vy -= fy * alpha;
  }

  // ── Centre gravity ─────────────────────────────────────────────────────────
  for (const n of nodes) {
    n.vx -= n.x * centerPull * alpha;
    n.vy -= n.y * centerPull * alpha;
  }

  // ── Integrate velocities ───────────────────────────────────────────────────
  let maxV2 = 0;
  for (const n of nodes) {
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

/**
 * Build the initial node array from raw graph data.
 *
 * Nodes are placed on a circle of radius √n · 30, with a small random jitter,
 * matching the layout chosen by `initSimulation` in GraphView.svelte.
 */
export function buildInitialNodes(
  rawNodes: { id: string; label: string }[],
  edgeCounts: Map<string, number>,
): SimNode[] {
  return rawNodes.map((n, i) => {
    const angle = (i / rawNodes.length) * Math.PI * 2;
    const radius = Math.sqrt(rawNodes.length) * 30;
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
}

/**
 * Merge a new set of raw nodes into an existing `SimNode[]`, preserving
 * positions for nodes that already exist (identified by `id`).
 *
 * New nodes are placed randomly near the origin, matching the behaviour of
 * `updateSimulation` in GraphView.svelte.
 */
export function mergeNodes(
  existing: SimNode[],
  rawNodes: { id: string; label: string }[],
  edgeCounts: Map<string, number>,
): SimNode[] {
  const posMap = new Map<string, { x: number; y: number }>();
  for (const n of existing) posMap.set(n.id, { x: n.x, y: n.y });

  return rawNodes.map((n) => {
    const pos = posMap.get(n.id);
    if (pos) {
      return {
        id: n.id,
        label: n.label,
        x: pos.x,
        y: pos.y,
        vx: 0,
        vy: 0,
        edges: edgeCounts.get(n.id) ?? 0,
      };
    }
    const angle = Math.random() * Math.PI * 2;
    const radius = 50 + Math.random() * 50;
    return {
      id: n.id,
      label: n.label,
      x: Math.cos(angle) * radius,
      y: Math.sin(angle) * radius,
      vx: 0,
      vy: 0,
      edges: edgeCounts.get(n.id) ?? 0,
    };
  });
}

/**
 * Build the `SimEdge[]` index-pair list from raw string IDs.
 */
export function buildEdges(
  rawEdges: { source: string; target: string }[],
  nodeIndex: Map<string, number>,
): SimEdge[] {
  const result: SimEdge[] = [];
  for (const e of rawEdges) {
    const si = nodeIndex.get(e.source);
    const ti = nodeIndex.get(e.target);
    if (si !== undefined && ti !== undefined && si !== ti) {
      result.push({ source: si, target: ti });
    }
  }
  return result;
}

/**
 * Cap a node list to the `limit` most-connected nodes and filter edges to
 * only those connecting the surviving nodes.
 *
 * This keeps the O(n²) fallback repulsion bounded even on large vaults.
 * Mirrors the capping logic in `initSimulation` / `updateSimulation`.
 */
export function capByDegree<
  N extends { id: string },
  E extends { source: string; target: string },
>(nodes: N[], edges: E[], limit: number): { nodes: N[]; edges: E[] } {
  if (nodes.length <= limit) return { nodes, edges };

  const edgeCount = new Map<string, number>();
  for (const e of edges) {
    edgeCount.set(e.source, (edgeCount.get(e.source) ?? 0) + 1);
    edgeCount.set(e.target, (edgeCount.get(e.target) ?? 0) + 1);
  }
  const cappedNodes = [...nodes]
    .sort((a, b) => (edgeCount.get(b.id) ?? 0) - (edgeCount.get(a.id) ?? 0))
    .slice(0, limit);
  const cappedIds = new Set(cappedNodes.map((n) => n.id));
  const cappedEdges = edges.filter(
    (e) => cappedIds.has(e.source) && cappedIds.has(e.target),
  );
  return { nodes: cappedNodes, edges: cappedEdges };
}

// ---------------------------------------------------------------------------
// Note on coordinate transforms (NOT shared between the two components)
// ---------------------------------------------------------------------------
//
// GraphView.svelte  – camera is centred in the viewport:
//
//   screenToWorld(sx, sy):
//     wx = (sx - containerWidth/2  - camX) / camScale
//     wy = (sy - containerHeight/2 - camY) / camScale
//
//   camX / camY are pixel offsets from the screen centre (positive = right/down).
//
// CanvasEditor.svelte – camera tracks the world-space top-left corner:
//
//   screenToWorld(sx, sy):
//     x = (sx - rect.left) / zoom + camX
//     y = (sy - rect.top)  / zoom + camY
//
//   worldToLocal(wx, wy):
//     x = (wx - camX) * zoom
//     y = (wy - camY) * zoom
//
//   camX / camY are world-space coordinates (they grow as you scroll right/down).
//
// These two models are structurally incompatible, so no shared helper is
// provided here. Each component should keep its own pair of transform helpers.
