import type { OpenClawConfig } from "../config/config.js";

function normalizeAgentId(value: string | undefined | null): string {
  const raw = (value ?? "").trim().toLowerCase();
  return raw || "main";
}

export function resolveDefaultAgentId(cfg: OpenClawConfig): string {
  const list = Array.isArray(cfg.agents?.list) ? cfg.agents?.list : [];
  if (list.length === 0) {
    return "main";
  }
  const defaultEntry = list.find((agent) => agent?.default);
  return normalizeAgentId((defaultEntry ?? list[0])?.id);
}
