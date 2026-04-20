// In Tauri 2, custom URI schemes on Windows/Android are served under
// `http://<scheme>.localhost/…`. On other platforms the raw scheme form
// is used. Keep these two prefixes in sync with text_transform.rs and
// the CSP in tauri.conf.json.

const isWindowsLike = (() => {
  if (typeof navigator === "undefined") return false;
  const platform =
    (navigator as any).userAgentData?.platform ??
    navigator.platform ??
    "";
  // Android and Windows both use the http.localhost form in Tauri 2.
  return /Win|Android/i.test(platform) || /Windows|Android/i.test(navigator.userAgent);
})();

export const LOCALFILE_URL_PREFIX = isWindowsLike
  ? "http://localfile.localhost"
  : "localfile://localhost";

const LEGACY_PREFIXES = [
  "http://localfile.localhost",
  "localfile://localhost",
];

/** True if `url` points to a vault file via either prefix form. */
export function isLocalfileUrl(url: string): boolean {
  return LEGACY_PREFIXES.some((p) => url.startsWith(p));
}

/** Strip either localfile prefix and return the remaining path (may start with `/`). */
export function stripLocalfilePrefix(url: string): string | null {
  for (const p of LEGACY_PREFIXES) {
    if (url.startsWith(p)) return url.slice(p.length);
  }
  return null;
}

/** Build an image src for a vault-absolute path. Spaces are %20-encoded. */
export function buildLocalfileUrl(absPath: string): string {
  const prefix = absPath.startsWith("/") ? "" : "/";
  return `${LOCALFILE_URL_PREFIX}${prefix}${absPath.replace(/ /g, "%20")}`;
}
