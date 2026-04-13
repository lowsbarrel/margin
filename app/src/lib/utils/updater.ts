import { toast } from "$lib/stores/toast.svelte";
import * as m from "$lib/paraglide/messages.js";

let updateShown = false;

export async function checkForAppUpdate() {
  try {
    const { check } = await import("@tauri-apps/plugin-updater");
    const update = await check();
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
  } catch {
    // Not running in Tauri or check failed — ignore
  }
}

async function installUpdate() {
  toast.push(m.update_installing(), "info");
  try {
    const { check } = await import("@tauri-apps/plugin-updater");
    const update = await check();
    if (update) {
      await update.downloadAndInstall();
      const { relaunch } = await import("@tauri-apps/plugin-process");
      await relaunch();
    }
  } catch {
    updateShown = false;
    toast.error(m.toast_update_failed());
  }
}
