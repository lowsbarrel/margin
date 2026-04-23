import { encryptBlob, decryptBlob } from "$lib/crypto/bridge";
import {
  readFileBytes,
  writeFileBytes,
  fileExists,
  createDirectory,
} from "$lib/fs/bridge";

// ── Types ──

export interface ManifestEntry {
  path: string;
  hash: string;
  /** Seconds since UNIX epoch. */
  modified: number;
  /** Seconds since UNIX epoch. */
  deleted_at?: number;
}

export interface Manifest {
  version: number;
  files: ManifestEntry[];
}

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
  }
  return obj as Manifest;
}

// ─── Base manifest (local) ──────────────────────────────────────────────

const BASE_MANIFEST_FILE = "sync-base.enc";

export async function loadBaseManifest(
  vaultPath: string,
  encryptionKey: number[],
): Promise<Manifest> {
  const path = `${vaultPath}/.margin/${BASE_MANIFEST_FILE}`;
  try {
    if (!(await fileExists(path))) return { version: 3, files: [] };
    const enc = await readFileBytes(path);
    const dec = await decryptBlob(enc, encryptionKey);
    return validateManifest(JSON.parse(new TextDecoder().decode(dec)));
  } catch {
    return { version: 3, files: [] };
  }
}

export async function saveBaseManifest(
  vaultPath: string,
  encryptionKey: number[],
  manifest: Manifest,
): Promise<void> {
  const json = new TextEncoder().encode(JSON.stringify(manifest));
  const enc = await encryptBlob(json, encryptionKey);
  await createDirectory(`${vaultPath}/.margin`);
  await writeFileBytes(`${vaultPath}/.margin/${BASE_MANIFEST_FILE}`, enc);
}
