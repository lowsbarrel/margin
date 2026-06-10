/** Per-path write serializer — one write per path in flight; new writes replace queued content. */

type WriteFn = (path: string, content: Uint8Array) => Promise<void>;

interface Pending {
  content: Uint8Array;
  resolve: () => void;
  reject: (err: unknown) => void;
}

const inflight = new Map<string, boolean>();
const pending = new Map<string, Pending>();

/**
 * Per-path promise that resolves when that path has no in-flight or pending
 * write left (i.e. the final queued content has actually landed on disk).
 * `flushWriteQueue()` awaits these so callers can ensure durable persistence
 * before the window closes.
 */
const settled = new Map<string, Promise<void>>();
const settleResolvers = new Map<string, () => void>();

let _rawWrite: WriteFn;

export function initWriteQueue(rawWrite: WriteFn): void {
  _rawWrite = rawWrite;
}

/**
 * Queue a write for `path`. Only one write per path is in flight at a time;
 * while one is in flight, a newer call replaces any queued content (latest
 * wins) and the superseded call's promise is resolved immediately.
 *
 * Resolution contract: the returned promise resolving means "a write at least
 * as new as yours completed" — NOT "your exact bytes were persisted". A newer
 * call can supersede yours, in which case you resolve before its content lands.
 * Callers that need byte-exact persistence (read-back, hash, mtime tied to the
 * content) must not rely on this. Use flushWriteQueue() to await the actual
 * final write for all paths.
 */
export function queuedWrite(path: string, content: Uint8Array): Promise<void> {
  if (!_rawWrite) throw new Error("writeQueue not initialised");

  // If nothing is in flight, run immediately
  if (!inflight.get(path)) {
    return runWrite(path, content);
  }

  // A write is in flight — replace pending content (latest wins)
  const prev = pending.get(path);
  if (prev) {
    // Resolve the old pending promise silently (its content was superseded)
    prev.resolve();
  }

  return new Promise<void>((resolve, reject) => {
    pending.set(path, { content, resolve, reject });
  });
}

/**
 * Resolve when every path's in-flight AND pending writes have completed.
 * Awaits the actually-final write per path (not a superseded one), so after
 * this resolves the disk holds the latest queued content for all paths.
 * Intended to be awaited on window close / before quitting.
 */
export async function flushWriteQueue(): Promise<void> {
  // A drain can chain a new write (and thus extend the settle window), so loop
  // until no path has any outstanding work.
  while (settled.size > 0) {
    await Promise.allSettled(Array.from(settled.values()));
  }
}

/** Mark `path` as having outstanding work, creating its settle promise once. */
function beginSettle(path: string): void {
  if (settled.has(path)) return;
  settled.set(
    path,
    new Promise<void>((res) => settleResolvers.set(path, res)),
  );
}

/** Mark `path` as fully drained: resolve and clear its settle promise. */
function endSettle(path: string): void {
  const res = settleResolvers.get(path);
  settleResolvers.delete(path);
  settled.delete(path);
  res?.();
}

async function runWrite(path: string, content: Uint8Array): Promise<void> {
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
    // Chain fully drained — the final content for this path has landed.
    endSettle(path);
    return;
  }
  pending.delete(path);
  runWrite(path, next.content).then(next.resolve, next.reject);
}
