import { vault } from '$lib/stores/vault.svelte';
import { walkDirectory, listAllLinks } from '$lib/fs/bridge';

export interface GraphNode {
	id: string;
	label: string;
}

export interface GraphEdge {
	source: string;
	target: string;
}

export interface GraphData {
	nodes: GraphNode[];
	edges: GraphEdge[];
}

interface GraphState {
	data: GraphData;
	loading: boolean;
	nodeToPath: Map<string, string>;
}

const state = $state<GraphState>({
	data: { nodes: [], edges: [] },
	loading: false,
	nodeToPath: new Map()
});

// Set when build() is called while a build is already in flight, so the request
// isn't lost (e.g. a vault-fs-changed during a build) — it re-runs once after.
let rebuildRequested = false;

function fileNameToId(name: string): string {
	return name.endsWith('.md') ? name.slice(0, -3) : name;
}

/**
 * Everything below builds plain `Map`/`Set` instances rather than the reactive
 * `SvelteMap`/`SvelteSet`, on purpose:
 *
 *  - `titleToAbs`, `linksByPath` and the node-id de-duplication are scratch
 *    collections local to one build. Nothing outside this function ever sees
 *    them, so there is nothing to make reactive.
 *  - `state.nodeToPath` is only ever *replaced* wholesale at the end of a build,
 *    never mutated afterwards. Its single consumer does one `.get()` per node
 *    click, so per-key reactive sources for a vault-sized lookup table would be
 *    pure overhead — the one dependency on the container is enough.
 *
 * They live in module scope rather than inside the exported `graph` object so
 * that stays structurally obvious.
 */
async function _build(): Promise<void> {
	const vaultPath = vault.vaultPath;
	if (!vaultPath) return;
	// Coalesce: if a build is already running, remember that another rebuild was
	// requested and re-run it once the current one finishes, so an fs change
	// during a build isn't silently dropped.
	if (state.loading) {
		rebuildRequested = true;
		return;
	}

	state.loading = true;
	try {
		const allEntries = await walkDirectory(vaultPath);
		const mdFiles = allEntries.filter((e) => !e.is_dir && e.name.endsWith('.md'));

		const titleToAbs = new Map(mdFiles.map((f) => [fileNameToId(f.name).toLowerCase(), f.path]));

		// One query for every link in the vault. This used to be a per-file
		// {mtime, links} cache at .margin/graph-cache.json plus a re-read of each
		// stale file; the search index already stores links, so both are gone.
		const linkEntries = await listAllLinks(vaultPath);
		const linksByPath = new Map(linkEntries.map((e) => [e.path, e.links]));

		// Node ids in first-seen order, duplicates included; de-duplicated in one
		// pass at the end. A Set built from this array keeps that same order, so
		// the node list is identical to appending into a Set as we go.
		const nodeIds: string[] = [];
		const edges: GraphEdge[] = [];

		for (const f of mdFiles) {
			const sourceId = fileNameToId(f.name);
			nodeIds.push(sourceId);
			for (const link of linksByPath.get(f.path) ?? []) {
				nodeIds.push(link);
				edges.push({ source: sourceId, target: link });
			}
		}

		const nodes: GraphNode[] = [...new Set(nodeIds)].map((id) => ({ id, label: id }));
		state.data = { nodes, edges };
		state.nodeToPath = titleToAbs;
	} finally {
		state.loading = false;
	}

	// A rebuild was requested while this build was running — run it now.
	if (rebuildRequested) {
		rebuildRequested = false;
		await _build();
	}
}

function _clear(): void {
	rebuildRequested = false;
	state.data = { nodes: [], edges: [] };
	state.nodeToPath = new Map();
}

export const graph = {
	get data() {
		return state.data;
	},
	get loading() {
		return state.loading;
	},
	get nodeToPath() {
		return state.nodeToPath;
	},

	build: _build,
	clear: _clear
};
