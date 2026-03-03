const BLOCKED_KEYS = new Set(["__proto__", "prototype", "constructor"]);

export function isBlockedObjectKey(value: string): boolean {
  return BLOCKED_KEYS.has((value ?? "").trim().toLowerCase());
}
