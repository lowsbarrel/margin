import { toast } from "$lib/stores/toast.svelte";

/** Run async op, show toast on error. Returns result or null. */
export async function withToast<T>(
  fn: () => Promise<T>,
  errorMsg?: string,
): Promise<T | null> {
  try {
    return await fn();
  } catch (err) {
    toast.error(errorMsg ?? String(err));
    return null;
  }
}
