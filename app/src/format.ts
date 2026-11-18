// Small helpers for showing things in plain words.

/** "1 file", "3 files". */
export function count(n: number, noun: string, plural = `${noun}s`): string {
  return `${n} ${n === 1 ? noun : plural}`;
}

export function size(bytes: number): string {
  if (bytes < 1024) return `${bytes} B`;
  if (bytes < 1024 * 1024) return `${(bytes / 1024).toFixed(1)} KB`;
  return `${(bytes / (1024 * 1024)).toFixed(1)} MB`;
}

/** An RFC 3339 time from the engine, in the user's own format. */
export function when(rfc3339: string): string {
  const date = new Date(rfc3339);
  if (Number.isNaN(date.getTime())) return rfc3339;
  return date.toLocaleString(undefined, {
    dateStyle: "long",
    timeStyle: "short",
  });
}

/** "A", "A and B", "A, B and C". */
export function list(items: string[]): string {
  if (items.length <= 1) return items.join("");
  return `${items.slice(0, -1).join(", ")} and ${items[items.length - 1]}`;
}

/**
 * Splits engine messages on `backticks`, so the quoted parts (a shortcut,
 * a path) can be shown as code: odd-numbered parts are the quoted ones.
 */
export function quoted(message: string): string[] {
  return message.split("`");
}

/** A name for a new settings file, such as "Settings 2026-11-17.zip". */
export function bundleName(now = new Date()): string {
  const pad = (n: number) => String(n).padStart(2, "0");
  return `Settings ${now.getFullYear()}-${pad(now.getMonth() + 1)}-${pad(now.getDate())}.zip`;
}

export function capitalize(text: string): string {
  return text.charAt(0).toUpperCase() + text.slice(1);
}
