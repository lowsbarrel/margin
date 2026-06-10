import { toast } from "$lib/stores/toast.svelte";
import * as m from "$lib/paraglide/messages.js";
import type { Update } from "@tauri-apps/plugin-updater";

let updateShown = false;
/** The Update handle returned by the first successful check(), reused on install. */
let pendingUpdate: Update | null = null;

/**
 * True when the updater plugin is unavailable, i.e. we are not running inside
 * the Tauri webview (e.g. dev in a plain browser). Such failures are expected
 * and silently ignored, whereas any other error is logged.
 */
function isNotInTauri(err: unknown): boolean {
  const message = err instanceof Error ? err.message : String(err);
  return (
    message.includes("not allowed") ||
    message.includes("__TAURI") ||
    message.includes("window.__TAURI_INTERNALS__") ||
    message.includes("Cannot read properties of undefined")
  );
}

export async function checkForAppUpdate() {
  try {
    const { check } = await import("@tauri-apps/plugin-updater");
    const update = await check();
    pendingUpdate = update;
    if (update && !updateShown) {
      updateShown = true;
      toast.push(
        m.update_available({ version: update.version }),
        "info",
        0, // persistent — no auto-dismiss
        {
          label: m.update_btn(),
          onClick: () => installUpdate(),
        },
      );
    }
  } catch (err) {
    // Not running in Tauri is expected; log anything else for diagnostics.
    if (!isNotInTauri(err)) {
      console.warn("Update check failed:", err);
    }
  }
}

async function installUpdate() {
  toast.push(m.update_installing(), "info");
  try {
    // Reuse the handle captured during checkForAppUpdate instead of issuing a
    // second check() round trip.
    const update = pendingUpdate;
    if (update) {
      await update.downloadAndInstall();
      const { relaunch } = await import("@tauri-apps/plugin-process");
      await relaunch();
    }
  } catch (err) {
    console.warn("Update install failed:", err);
    updateShown = false;
    pendingUpdate = null;
    toast.error(m.toast_update_failed());
  }
}
