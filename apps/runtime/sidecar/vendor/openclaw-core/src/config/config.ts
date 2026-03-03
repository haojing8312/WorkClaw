export type OpenClawBinding = {
  agentId: string;
  comment?: string;
  match: {
    channel: string;
    accountId?: string;
    peer?: { kind: "direct" | "group" | "channel"; id: string };
    guildId?: string;
    teamId?: string;
    roles?: string[];
  };
};

export type OpenClawConfig = {
  agents?: {
    list?: Array<{ id: string; default?: boolean }>;
  };
  bindings?: OpenClawBinding[];
  session?: {
    dmScope?: "main" | "per-peer" | "per-channel-peer" | "per-account-channel-peer";
    identityLinks?: Record<string, string[]>;
  };
};
