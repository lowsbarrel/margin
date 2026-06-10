// ── Types ──
//
// The boundary structs are generated from Rust by tauri-specta. specta emits a
// separate Serialize/Deserialize variant for each (the Serialize variant has an
// optional `deleted_at`). The app constructs manifests, so it uses the
// Serialize variant; re-exported here so existing importers keep working.

export type { ManifestEntry_Serialize as ManifestEntry } from "$lib/bindings";
export type { Manifest_Serialize as Manifest } from "$lib/bindings";
import type {
  Manifest_Serialize as Manifest,
  ManifestEntry_Serialize as ManifestEntry,
} from "$lib/bindings";

export function validateManifest(obj: unknown): Manifest {
  if (typeof obj !== "object" || obj === null)
    throw new Error("Manifest is not an object");
  const m = obj as Record<string, unknown>;
  if (typeof m.version !== "number")
    throw new Error("Manifest missing version");
  if (!Array.isArray(m.files)) throw new Error("Manifest missing files array");
  for (const entry of m.files) {
    if (typeof entry !== "object" || entry === null)
      throw new Error("Invalid manifest entry");
    const e = entry as Record<string, unknown>;
    if (typeof e.path !== "string")
      throw new Error("Manifest entry missing path");
    if (typeof e.hash !== "string")
      throw new Error("Manifest entry missing hash");
    if (typeof e.modified !== "number")
      throw new Error("Manifest entry missing modified");
    // deleted_at is optional, but if present it must be a number — this is the
    // decrypted-JSON trust boundary feeding the 3-way sync diff.
    if (e.deleted_at !== undefined && typeof e.deleted_at !== "number")
      throw new Error("Manifest entry has invalid deleted_at");
  }
  return obj as Manifest;
}
