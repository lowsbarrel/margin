// One-shot "Clean vault" maintenance: rewrites every note that still contains
// legacy non-Markdown appearance (text colour / highlight / underline / sub-sup
// HTML, and `==highlight==`) into its clean, fully-Markdown form.
//
// It does this with a single throwaway headless editor, mirroring the exact
// load → save round-trip a normal edit performs (resolve image paths → parse →
// re-serialize → unresolve), so the output is byte-identical to what the app
// would write if you opened and saved the note. Only files that match the
// legacy markers are touched; everything else is left untouched on disk. The
// original of each rewritten file is snapshotted to history first as a restore
// point.

import { Editor } from "@tiptap/core";
import { common, createLowlight } from "lowlight";
import { createEditorExtensions } from "$lib/editor/extensions";
import { transformImagePaths } from "$lib/editor/text-transform-bridge";
import { unresolveImagePaths } from "$lib/editor/image-paths";
import { walkDirectory, readFileBytes, writeFileBytes } from "$lib/fs/bridge";
import { saveSnapshot } from "$lib/history/bridge";
import { loadSettings } from "$lib/settings/bridge";

// Mirrors the markers the per-open migration used. A cheap gate — the real
// decision is whether the cleaned re-serialization actually differs from disk.
const LEGACY_APPEARANCE_REGEX =
  /==[^=\n]+==|<\/?(?:u|sub|sup|mark)\b|<span[^>]*\sstyle\s*=|style\s*=\s*["'][^"']*(?:color|background)/i;

export interface CleanVaultProgress {
  scanned: number;
  total: number;
  cleaned: number;
}

export interface CleanVaultResult {
  scanned: number;
  cleaned: number;
}

function getEditorMarkdown(e: Editor): string {
  return (e.storage as { markdown?: { getMarkdown?: () => string } }).markdown
    ?.getMarkdown?.() ?? "";
}

export async function cleanVault(
  vaultPath: string,
  encryptionKey: number[] | null,
  onProgress?: (p: CleanVaultProgress) => void,
): Promise<CleanVaultResult> {
  // Resolve the attachment folder exactly as the editor does, so image paths
  // round-trip unchanged and the only diff is the stripped appearance.
  let attachmentFolder: string | null = null;
  if (encryptionKey) {
    try {
      const settings = await loadSettings(vaultPath, encryptionKey);
      attachmentFolder = settings?.attachment_folder ?? null;
    } catch {
      /* fall back to the default attachment folder */
    }
  }

  // `.margin/` (history, workspace, sync base) is hidden, so it's excluded.
  const entries = await walkDirectory(vaultPath);
  const mdFiles = entries.filter(
    (e) => !e.is_dir && e.path.toLowerCase().endsWith(".md"),
  );
  const total = mdFiles.length;

  const lowlight = createLowlight(common);
  // Off-screen host: NodeViews (math/mermaid/images) get a layout context but
  // nothing is visible. Removed in `finally`.
  const host = document.createElement("div");
  host.style.cssText =
    "position:fixed;left:-99999px;top:0;width:800px;height:0;overflow:hidden;opacity:0;pointer-events:none;";
  document.body.appendChild(host);

  const editor = new Editor({
    element: host,
    extensions: createEditorExtensions({ lowlight, attachmentFolder, embed: true }),
    content: "",
  });

  const decoder = new TextDecoder();
  const encoder = new TextEncoder();
  let scanned = 0;
  let cleaned = 0;

  try {
    for (const file of mdFiles) {
      scanned++;
      try {
        const bytes = await readFileBytes(file.path);
        const raw = decoder.decode(bytes);
        if (LEGACY_APPEARANCE_REGEX.test(raw)) {
          const resolved = await transformImagePaths(
            raw,
            vaultPath,
            attachmentFolder,
            "resolve",
          );
          editor.commands.setContent(resolved, { emitUpdate: false });
          const out = unresolveImagePaths(getEditorMarkdown(editor), vaultPath);
          if (out !== raw) {
            await saveSnapshot(vaultPath, file.path, bytes); // restore point
            await writeFileBytes(file.path, encoder.encode(out));
            cleaned++;
          }
        }
      } catch (err) {
        console.warn("clean-vault: skipped", file.path, err);
      }

      if (onProgress && (scanned % 5 === 0 || scanned === total)) {
        onProgress({ scanned, total, cleaned });
      }
      // Yield periodically so the UI stays responsive on large vaults.
      if (scanned % 25 === 0) await new Promise((r) => setTimeout(r, 0));
    }
  } finally {
    editor.destroy();
    host.remove();
  }

  return { scanned, cleaned };
}
