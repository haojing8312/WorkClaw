export type RoutePeerKind = "direct" | "group" | "channel";

export type RoutePeer = {
  kind: RoutePeerKind;
  id: string;
};

export type RouteBinding = {
  agentId: string;
  comment?: string;
  match: {
    channel: string;
    accountId?: string;
    peer?: RoutePeer;
    guildId?: string;
    teamId?: string;
    roles?: string[];
  };
};

export type RouteInput = {
  channel: string;
  accountId?: string | null;
  peer?: RoutePeer | null;
  parentPeer?: RoutePeer | null;
  guildId?: string | null;
  teamId?: string | null;
  memberRoleIds?: string[];
  bindings: RouteBinding[];
  defaultAgentId: string;
  dmScope?: "main" | "per-peer" | "per-channel-peer" | "per-account-channel-peer";
  identityLinks?: Record<string, string[]>;
};

export type RouteOutput = {
  agentId: string;
  channel: string;
  accountId: string;
  sessionKey: string;
  mainSessionKey: string;
  matchedBy:
    | "binding.peer"
    | "binding.peer.parent"
    | "binding.guild+roles"
    | "binding.guild"
    | "binding.team"
    | "binding.account"
    | "binding.channel"
    | "default";
};
