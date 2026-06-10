/**
 * Small typed helpers for marshalling binary payloads across the Tauri IPC
 * boundary. Tauri serializes byte arrays as JSON `number[]`, so these convert
 * between `Uint8Array` and the wire representation in one place instead of
 * hand-rolling `Array.from` / `new Uint8Array` in every bridge.
 */

/** Convert a `Uint8Array` to the `number[]` shape expected over IPC. */
export const toBytes = (u: Uint8Array): number[] => Array.from(u);

/** Convert an IPC byte payload (`number[]` or `ArrayBuffer`) back to a `Uint8Array`. */
export const fromBytes = (a: number[] | ArrayBuffer): Uint8Array =>
  a instanceof ArrayBuffer ? new Uint8Array(a) : Uint8Array.from(a);
