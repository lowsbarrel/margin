/**
 * Per-path write serializer.
 *
 * Guarantees that for any given file path, only one write is in flight at a time.
 * If a new write is requested while one is already running, the new content
 * replaces any previously queued content and executes immediately after the
 * current write finishes — no delays, no debouncing.
 */

type WriteFn = (path: string, content: Uint8Array) => Promise<void>;

interface Pending {
  content: Uint8Array;
  resolve: () => void;
  reject: (err: unknown) => void;
}

const inflight = new Map<string, boolean>();
const pending = new Map<string, Pending>();

let _rawWrite: WriteFn;

export function initWriteQueue(rawWrite: WriteFn): void {
  _rawWrite = rawWrite;
}

/**
 * Enqueue a write for the given path.
 * - If no write is in flight for this path, starts immediately.
 * - If a write IS in flight, replaces any previously pending content
 *   so only the latest data is written when the current write finishes.
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

async function runWrite(path: string, content: Uint8Array): Promise<void> {
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
  if (!next) return;
  pending.delete(path);
  runWrite(path, next.content).then(next.resolve, next.reject);
}
