import { toast } from "$lib/stores/toast.svelte";

/** Run async op, show toast on error. Returns result or null. */
export async function withToast<T>(
  fn: () => Promise<T>,
  errorMsg?: string,
): Promise<T | null> {
  try {
    return await fn();
  } catch (err) {
    // Always log the raw error for diagnostics; surface a clean user-facing
    // message instead of String(err) (which can render "[object Object]").
    console.error(err);
    toast.error(errorMsg ?? "Something went wrong. Please try again.");
    return null;
  }
}
