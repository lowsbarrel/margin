type WriteFn = (path: string, content: Uint8Array) => Promise<void>;

interface Pending {
	content: Uint8Array;
	resolve: () => void;
	reject: (err: unknown) => void;
}

const inflight = new Map<string, boolean>();
const pending = new Map<string, Pending>();

const settled = new Map<string, Promise<void>>();
const settleResolvers = new Map<string, () => void>();

let _rawWrite: WriteFn;

export function initWriteQueue(rawWrite: WriteFn): void {
	_rawWrite = rawWrite;
}

const RECENT_WRITES_PER_PATH = 3;
const recentWrites = new Map<string, string[]>();

function fingerprint(bytes: Uint8Array): string {
	let a = 0x811c9dc5;
	let b = 0x01000193;
	for (let i = 0; i < bytes.length; i++) {
		const byte = bytes[i];
		a = ((a ^ byte) * 0x01000193) >>> 0;
		b = ((b ^ byte) * 0x811c9dc5) >>> 0;
	}
	return `${bytes.length}:${a.toString(36)}:${b.toString(36)}`;
}

function recordRecentWrite(path: string, content: Uint8Array): void {
	const digest = fingerprint(content);
	const list = recentWrites.get(path) ?? [];
	if (list[list.length - 1] === digest) return;
	list.push(digest);
	if (list.length > RECENT_WRITES_PER_PATH) list.shift();
	recentWrites.set(path, list);
}

export function isOwnRecentWrite(path: string, content: Uint8Array): boolean {
	return recentWrites.get(path)?.includes(fingerprint(content)) ?? false;
}

export function remapRecentWrites(from: string, to: string): void {
	for (const [path, list] of Array.from(recentWrites)) {
		if (path === from) {
			recentWrites.delete(path);
			recentWrites.set(to, list);
		} else if (path.startsWith(`${from}/`)) {
			recentWrites.delete(path);
			recentWrites.set(to + path.slice(from.length), list);
		}
	}
}

// margin:flush listeners are synchronous, so every write enqueues before the drain.
export async function flushEditorWrites(): Promise<void> {
	window.dispatchEvent(new Event('margin:flush'));
	await flushWriteQueue();
}

// Resolving means a write at least as new landed — not that these bytes did.
export function queuedWrite(path: string, content: Uint8Array): Promise<void> {
	if (!_rawWrite) throw new Error('writeQueue not initialised');

	if (!inflight.get(path)) {
		return runWrite(path, content);
	}

	const prev = pending.get(path);
	if (prev) {
		prev.resolve();
	}

	return new Promise<void>((resolve, reject) => {
		pending.set(path, { content, resolve, reject });
	});
}

export async function flushWriteQueue(): Promise<void> {
	while (settled.size > 0) {
		await Promise.allSettled(Array.from(settled.values()));
	}
}

function beginSettle(path: string): void {
	if (settled.has(path)) return;
	settled.set(path, new Promise<void>((res) => settleResolvers.set(path, res)));
}

function endSettle(path: string): void {
	const res = settleResolvers.get(path);
	settleResolvers.delete(path);
	settled.delete(path);
	res?.();
}

async function runWrite(path: string, content: Uint8Array): Promise<void> {
	recordRecentWrite(path, content);
	beginSettle(path);
	inflight.set(path, true);
	try {
		await _rawWrite(path, content);
	} finally {
		inflight.delete(path);
		drain(path);
	}
}

function drain(path: string): void {
	const next = pending.get(path);
	if (!next) {
		endSettle(path);
		return;
	}
	pending.delete(path);
	runWrite(path, next.content).then(next.resolve, next.reject);
}
