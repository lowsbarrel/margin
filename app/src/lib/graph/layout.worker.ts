// Force-directed graph layout, run off the main thread.
//
// The worker owns the physics simulation (Barnes-Hut repulsion, edge
// attraction, center gravity) and operates purely on flat numeric arrays so
// positions can be posted back as a transferable Float32Array with no
// structured-clone cost. The main thread owns rendering and interaction and
// never touches physics, so layout of large graphs no longer blocks the UI.
//
// Protocol:
//   main -> worker:
//     { type: "init", count, positions: ArrayBuffer, edges: ArrayBuffer }
//     { type: "drag", index, x, y }   pin a node to a cursor position
//     { type: "dragEnd" }             release the pinned node
//     { type: "wake" }                resume ticking (e.g. on interaction)
//     { type: "stop" }                pause ticking
//   worker -> main:
//     { type: "positions", positions: ArrayBuffer, count }  (transferable)
//     { type: "settled" }             layout converged; main may park its raf

interface InitMsg {
	type: 'init';
	count: number;
	positions: ArrayBuffer; // Float32Array, interleaved x,y
	edges: ArrayBuffer; // Int32Array, interleaved source,target node indices
}
interface DragMsg {
	type: 'drag';
	index: number;
	x: number;
	y: number;
}
interface DragEndMsg {
	type: 'dragEnd';
}
interface WakeMsg {
	type: 'wake';
}
interface StopMsg {
	type: 'stop';
}
type InMsg = InitMsg | DragMsg | DragEndMsg | WakeMsg | StopMsg;

// `self` in a module worker is a DedicatedWorkerGlobalScope, but without the
// "webworker" lib (which conflicts with "dom") TS types it as a Window, whose
// postMessage signature differs. Narrow to just what we use.
interface WorkerCtx {
	postMessage(message: unknown, transfer?: Transferable[]): void;
	onmessage: ((ev: MessageEvent) => void) | null;
}
const ctx = self as unknown as WorkerCtx;

// ─── Simulation constants (match the previous main-thread values) ───────────
const ALPHA = 0.3;
const REPULSION = 800;
const ATTRACTION = 0.005;
const IDEAL_LENGTH = 100;
const DAMPING = 0.85;
const CENTER_PULL = 0.01;
const THETA = 0.8; // Barnes-Hut opening-angle threshold
const SLEEP_VELOCITY_SQ = 0.01; // park when all nodes slower than ~0.1px/frame
const TICK_MS = 16; // ~60 ticks/sec, matching one simulate() per frame before

// ─── Mutable simulation state ───────────────────────────────────────────────
let count = 0;
let px = new Float32Array(0);
let py = new Float32Array(0);
let vx = new Float32Array(0);
let vy = new Float32Array(0);
let edges = new Int32Array(0); // [s0,t0, s1,t1, ...]
let dragIndex = -1;
let running = false;
let timer: ReturnType<typeof setTimeout> | 0 = 0;

// ─── Barnes-Hut quadtree (index-based) ──────────────────────────────────────
interface QTNode {
	cx: number;
	cy: number;
	mass: number;
	x1: number;
	y1: number;
	x2: number;
	y2: number;
	children: (QTNode | null)[];
	body: number; // node index, or -1 for an internal/empty cell
}

function qtNew(x1: number, y1: number, x2: number, y2: number): QTNode {
	return { cx: 0, cy: 0, mass: 0, x1, y1, x2, y2, children: [null, null, null, null], body: -1 };
}

function qtInsert(qt: QTNode, i: number) {
	if (qt.mass === 0) {
		qt.body = i;
		qt.cx = px[i];
		qt.cy = py[i];
		qt.mass = 1;
		return;
	}
	if (qt.body !== -1) {
		// Subdivide: push the existing single body down a level.
		const existing = qt.body;
		qt.body = -1;
		qtPush(qt, existing);
	}
	qt.cx = (qt.cx * qt.mass + px[i]) / (qt.mass + 1);
	qt.cy = (qt.cy * qt.mass + py[i]) / (qt.mass + 1);
	qt.mass += 1;
	qtPush(qt, i);
}

function qtPush(qt: QTNode, i: number) {
	const mx = (qt.x1 + qt.x2) / 2;
	const my = (qt.y1 + qt.y2) / 2;
	const idx = (px[i] > mx ? 1 : 0) + (py[i] > my ? 2 : 0);
	let child = qt.children[idx];
	if (!child) {
		const x1 = idx & 1 ? mx : qt.x1;
		const y1 = idx & 2 ? my : qt.y1;
		const x2 = idx & 1 ? qt.x2 : mx;
		const y2 = idx & 2 ? qt.y2 : my;
		child = qtNew(x1, y1, x2, y2);
		qt.children[idx] = child;
	}
	qtInsert(child, i);
}

function qtApplyForce(qt: QTNode, i: number) {
	if (qt.mass === 0) return;
	const dx = qt.cx - px[i];
	const dy = qt.cy - py[i];
	const distSq = dx * dx + dy * dy || 1;
	const size = qt.x2 - qt.x1;

	if (qt.body !== -1) {
		if (qt.body === i) return; // skip self
		const dist = Math.sqrt(distSq);
		const force = REPULSION / distSq;
		vx[i] -= (dx / dist) * force * ALPHA;
		vy[i] -= (dy / dist) * force * ALPHA;
		return;
	}

	// Far enough away: approximate the whole cell by its center of mass.
	if ((size * size) / distSq < THETA * THETA) {
		const dist = Math.sqrt(distSq);
		const force = (REPULSION * qt.mass) / distSq;
		vx[i] -= (dx / dist) * force * ALPHA;
		vy[i] -= (dy / dist) * force * ALPHA;
		return;
	}

	for (const child of qt.children) {
		if (child) qtApplyForce(child, i);
	}
}

// ─── One simulation step ────────────────────────────────────────────────────
function tick() {
	timer = 0;

	// Bounding box for the quadtree root.
	let bx1 = Infinity;
	let by1 = Infinity;
	let bx2 = -Infinity;
	let by2 = -Infinity;
	for (let i = 0; i < count; i++) {
		const x = px[i];
		const y = py[i];
		if (x < bx1) bx1 = x;
		if (y < by1) by1 = y;
		if (x > bx2) bx2 = x;
		if (y > by2) by2 = y;
	}
	const pad = Math.max(1, (bx2 - bx1) * 0.01, (by2 - by1) * 0.01);
	const root = qtNew(bx1 - pad, by1 - pad, bx2 + pad, by2 + pad);
	for (let i = 0; i < count; i++) qtInsert(root, i);
	for (let i = 0; i < count; i++) qtApplyForce(root, i);

	// Attraction along edges.
	for (let e = 0; e < edges.length; e += 2) {
		const a = edges[e];
		const b = edges[e + 1];
		const dx = px[b] - px[a];
		const dy = py[b] - py[a];
		const dist = Math.sqrt(dx * dx + dy * dy) || 1;
		const force = (dist - IDEAL_LENGTH) * ATTRACTION;
		const fx = (dx / dist) * force;
		const fy = (dy / dist) * force;
		vx[a] += fx * ALPHA;
		vy[a] += fy * ALPHA;
		vx[b] -= fx * ALPHA;
		vy[b] -= fy * ALPHA;
	}

	// Center gravity.
	for (let i = 0; i < count; i++) {
		vx[i] -= px[i] * CENTER_PULL * ALPHA;
		vy[i] -= py[i] * CENTER_PULL * ALPHA;
	}

	// Integrate, keeping the dragged node pinned, and track peak energy.
	let maxV2 = 0;
	for (let i = 0; i < count; i++) {
		if (i === dragIndex) {
			vx[i] = 0;
			vy[i] = 0;
			continue;
		}
		vx[i] *= DAMPING;
		vy[i] *= DAMPING;
		px[i] += vx[i];
		py[i] += vy[i];
		const v2 = vx[i] * vx[i] + vy[i] * vy[i];
		if (v2 > maxV2) maxV2 = v2;
	}

	postPositions();

	if (maxV2 <= SLEEP_VELOCITY_SQ && dragIndex < 0) {
		running = false;
		ctx.postMessage({ type: 'settled' });
	} else {
		timer = setTimeout(tick, TICK_MS);
	}
}

function postPositions() {
	const out = new Float32Array(count * 2);
	for (let i = 0; i < count; i++) {
		out[2 * i] = px[i];
		out[2 * i + 1] = py[i];
	}
	ctx.postMessage({ type: 'positions', positions: out.buffer, count }, [out.buffer]);
}

function start() {
	if (running || count === 0) return;
	running = true;
	tick();
}

function stop() {
	running = false;
	if (timer) {
		clearTimeout(timer);
		timer = 0;
	}
}

ctx.onmessage = (ev: MessageEvent) => {
	const msg = ev.data as InMsg;
	switch (msg.type) {
		case 'init': {
			stop();
			count = msg.count;
			const pos = new Float32Array(msg.positions);
			px = new Float32Array(count);
			py = new Float32Array(count);
			vx = new Float32Array(count);
			vy = new Float32Array(count);
			for (let i = 0; i < count; i++) {
				px[i] = pos[2 * i];
				py[i] = pos[2 * i + 1];
			}
			edges = new Int32Array(msg.edges);
			dragIndex = -1;
			start();
			break;
		}
		case 'drag': {
			dragIndex = msg.index;
			if (dragIndex >= 0 && dragIndex < count) {
				px[dragIndex] = msg.x;
				py[dragIndex] = msg.y;
				vx[dragIndex] = 0;
				vy[dragIndex] = 0;
			}
			if (!running) start();
			break;
		}
		case 'dragEnd': {
			dragIndex = -1;
			if (!running) start();
			break;
		}
		case 'wake': {
			if (!running) start();
			break;
		}
		case 'stop': {
			stop();
			break;
		}
	}
};
