// Pure graph render-state helpers for GraphView.
//
// Everything here is deliberately NON-reactive, and lives outside the component
// to say so structurally: a plain `.ts` module cannot hold Svelte state at all.
// The graph is painted to a <canvas> from flat typed arrays, so none of these
// lookup structures is ever read through the reactivity graph — they are rebuilt
// wholesale (per graph rebuild, and per hovered frame for the highlight sets).
// Swapping them for SvelteMap/SvelteSet would allocate a signal per entry inside
// the draw loop to publish changes nothing subscribes to.
//
// The layout physics already lives next door in `layout.worker.ts`; this is the
// same split applied to the data structures the renderer needs.

import type { GraphNode, GraphEdge } from '$lib/stores/graph.svelte';

export interface GraphInput {
	nodes: GraphNode[];
	edges: GraphEdge[];
}

/** Flat render state, shared between the canvas draw loop and the layout worker. */
export interface FlatGraph {
	count: number;
	/** Interleaved [x0,y0, x1,y1, ...]. */
	pos: Float32Array;
	nodeIds: string[];
	nodeLabels: string[];
	/** Degree per node, for node sizing. */
	nodeDeg: Float32Array;
	/** Interleaved [source,target, ...] node-index pairs, for drawing. */
	edgePairs: Int32Array;
	/** Lowercased node id -> index. */
	nodeIndexLc: Map<string, number>;
}

/** Produces the initial world position for a node. */
export type SeedFn = (id: string, index: number, total: number) => [number, number];

/** The subset of a node's neighbourhood the draw loop paints differently. */
export interface Highlights {
	/** Start offsets into `edgePairs` (0, 2, 4, ...) of the connected edges. */
	edges: Set<number>;
	/** Indices of the hovered node and everything it links to. */
	nodes: Set<number>;
}

/**
 * Cap the graph to the `maxNodes` most-connected nodes, dropping edges that
 * reference a culled node.
 */
export function capGraph(data: GraphInput, maxNodes: number): GraphInput {
	if (data.nodes.length <= maxNodes) {
		return { nodes: data.nodes, edges: data.edges };
	}
	const edgeCount = new Map<string, number>();
	for (const e of data.edges) {
		edgeCount.set(e.source, (edgeCount.get(e.source) ?? 0) + 1);
		edgeCount.set(e.target, (edgeCount.get(e.target) ?? 0) + 1);
	}
	const nodes = [...data.nodes]
		.sort((a, b) => (edgeCount.get(b.id) ?? 0) - (edgeCount.get(a.id) ?? 0))
		.slice(0, maxNodes);
	const cappedIds = new Set(nodes.map((n) => n.id));
	const edges = data.edges.filter((e) => cappedIds.has(e.source) && cappedIds.has(e.target));
	return { nodes, edges };
}

/**
 * Build the flat render arrays for a graph, capping it first. `seed` supplies
 * each node's starting position — the only thing that differs between a fresh
 * layout (circle) and an incremental update (carry existing positions).
 */
export function buildFlatGraph(data: GraphInput, maxNodes: number, seed: SeedFn): FlatGraph {
	const { nodes, edges } = capGraph(data, maxNodes);
	const count = nodes.length;

	const nodeIds: string[] = new Array(count);
	const nodeLabels: string[] = new Array(count);
	const nodeDeg = new Float32Array(count);
	const nodeIndexLc = new Map<string, number>();
	const pos = new Float32Array(count * 2);

	const idToIndex = new Map<string, number>();
	for (let i = 0; i < count; i++) {
		const n = nodes[i];
		nodeIds[i] = n.id;
		nodeLabels[i] = n.label;
		nodeIndexLc.set(n.id.toLowerCase(), i);
		idToIndex.set(n.id, i);
		const [x, y] = seed(n.id, i, count);
		pos[2 * i] = x;
		pos[2 * i + 1] = y;
	}

	const pairs: number[] = [];
	for (const e of edges) {
		const s = idToIndex.get(e.source);
		const t = idToIndex.get(e.target);
		if (s !== undefined && t !== undefined && s !== t) {
			pairs.push(s, t);
			nodeDeg[s] += 1;
			nodeDeg[t] += 1;
		}
	}

	return {
		count,
		pos,
		nodeIds,
		nodeLabels,
		nodeDeg,
		edgePairs: Int32Array.from(pairs),
		nodeIndexLc
	};
}

/**
 * Snapshot the current positions by lowercased node id, so an incremental
 * rebuild can carry surviving nodes over instead of re-seeding the layout.
 */
export function snapshotPositions(
	count: number,
	nodeIds: string[],
	pos: Float32Array
): Map<string, [number, number]> {
	const prev = new Map<string, [number, number]>();
	for (let i = 0; i < count; i++) {
		prev.set(nodeIds[i].toLowerCase(), [pos[2 * i], pos[2 * i + 1]]);
	}
	return prev;
}

/** Find the edges and nodes connected to the hovered node (empty when none is). */
export function computeHighlights(edgePairs: Int32Array, hoverIndex: number): Highlights {
	const edges = new Set<number>();
	const nodes = new Set<number>();
	if (hoverIndex < 0) return { edges, nodes };

	nodes.add(hoverIndex);
	for (let i = 0; i < edgePairs.length; i += 2) {
		const s = edgePairs[i];
		const t = edgePairs[i + 1];
		if (s === hoverIndex || t === hoverIndex) {
			edges.add(i);
			nodes.add(s);
			nodes.add(t);
		}
	}
	return { edges, nodes };
}
