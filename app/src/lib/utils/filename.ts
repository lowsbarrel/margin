import * as m from "$lib/paraglide/messages.js";

const INVALID_CHARS = /[/\\:*?"<>|]/;
const NON_ASCII = /[^\x20-\x7E]/;
const RESERVED_NAMES = new Set([
  "CON",
  "PRN",
  "AUX",
  "NUL",
  "COM1",
  "COM2",
  "COM3",
  "COM4",
  "COM5",
  "COM6",
  "COM7",
  "COM8",
  "COM9",
  "LPT1",
  "LPT2",
  "LPT3",
  "LPT4",
  "LPT5",
  "LPT6",
  "LPT7",
  "LPT8",
  "LPT9",
]);

/** Returns error string if invalid, null if ok. */
export function validateName(name: string): string | null {
  const trimmed = name.trim();
  if (!trimmed) return m.validate_name_empty();
  if (NON_ASCII.test(trimmed))
    return m.validate_name_ascii();
  if (INVALID_CHARS.test(trimmed))
    return m.validate_name_invalid_chars();
  if (trimmed.startsWith(".")) return m.validate_name_dot_start();
  if (/[\s.]$/.test(trimmed)) return m.validate_name_trailing();
  const stem = trimmed.includes(".")
    ? trimmed.slice(0, trimmed.lastIndexOf("."))
    : trimmed;
  if (RESERVED_NAMES.has(stem.toUpperCase()))
    return m.validate_name_reserved({ name: trimmed });
  return null;
}

export function displayName(name: string): string {
  return name.replace(/\.(md|canvas)$/, "");
}

/** Ensure a name has .md extension. Returns null if invalid. */
export function ensureMdExtension(raw: string): string | null {
  const name = raw.trim();
  const error = validateName(name);
  if (error) return null;
  return name.includes(".") ? name : `${name}.md`;
}
