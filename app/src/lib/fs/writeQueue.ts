/** Per-path write serializer — one write per path in flight; new writes replace queued content. */

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
