export type ParsedAgentSessionKey = {
  agentId: string;
  rest: string;
};

export function parseAgentSessionKey(
  rawSessionKey: string | undefined | null,
): ParsedAgentSessionKey | null {
  const value = (rawSessionKey ?? "").trim().toLowerCase();
  if (!value.startsWith("agent:")) {
    return null;
  }
  const parts = value.split(":");
  if (parts.length < 3) {
    return null;
  }
  const agentId = (parts[1] ?? "").trim();
  const rest = parts.slice(2).join(":").trim();
  if (!agentId || !rest) {
    return null;
  }
  return { agentId, rest };
}

export function isSubagentSessionKey(_sessionKey: string | undefined | null): boolean {
  return false;
}

export function getSubagentDepth(_sessionKey: string | undefined | null): number {
  return 0;
}

export function isCronSessionKey(_sessionKey: string | undefined | null): boolean {
  return false;
}

export function isAcpSessionKey(_sessionKey: string | undefined | null): boolean {
  return false;
}
