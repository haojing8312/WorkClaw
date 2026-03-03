import { resolveAgentRoute } from "../../vendor/openclaw-core/src/routing/resolve-route.js";
import type { RouteInput, RouteOutput } from "./types.js";

function normalizeBindings(bindings: RouteInput["bindings"]): RouteInput["bindings"] {
  return Array.isArray(bindings) ? bindings : [];
}

function buildAgentList(input: RouteInput): Array<{ id: string; default?: boolean }> {
  const defaultAgentId = input.defaultAgentId?.trim() || "main";
  const unique = new Set<string>([defaultAgentId]);
  for (const binding of normalizeBindings(input.bindings)) {
    const id = binding?.agentId?.trim();
    if (id) {
      unique.add(id);
    }
  }
  return Array.from(unique).map((id) => ({ id, default: id === defaultAgentId }));
}

export function resolveRoute(input: RouteInput): RouteOutput {
  const cfg = {
    agents: { list: buildAgentList(input) },
    bindings: normalizeBindings(input.bindings),
    session: {
      dmScope: input.dmScope,
      identityLinks: input.identityLinks,
    },
  };
  return resolveAgentRoute({
    cfg,
    channel: input.channel,
    accountId: input.accountId,
    peer: input.peer,
    parentPeer: input.parentPeer,
    guildId: input.guildId,
    teamId: input.teamId,
    memberRoleIds: input.memberRoleIds,
  });
}
