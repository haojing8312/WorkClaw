import * as Lark from '@larksuiteoapi/node-sdk';

export interface FeishuWsStartInput {
  employee_id?: string;
  app_id?: string;
  app_secret?: string;
}

export interface FeishuWsReconcileItem {
  employee_id: string;
  app_id?: string;
  app_secret?: string;
}

export interface FeishuWsEventRecord {
  employee_id: string;
  id: string;
  event_type: string;
  chat_id: string;
  message_id: string;
  text: string;
  mention_open_id: string;
  sender_open_id: string;
  received_at: string;
  raw: unknown;
}

export interface FeishuWsStatus {
  running: boolean;
  started_at: string | null;
  queued_events: number;
}

export interface FeishuEmployeeWsStatus extends FeishuWsStatus {
  employee_id: string;
  last_event_at: string | null;
  last_error: string | null;
  reconnect_attempts: number;
}

export interface FeishuWsStatusSummary extends FeishuWsStatus {
  items: FeishuEmployeeWsStatus[];
  running_count: number;
}

type WsClientLike = {
  start: (options: { eventDispatcher: unknown }) => void;
  stop?: () => void;
  close?: () => void;
};

type EventDispatcherLike = {
  register: (handlers: Record<string, (payload: unknown) => Promise<unknown>>) => unknown;
};

type LarkSdkLike = {
  WSClient: new (config: Record<string, unknown>) => WsClientLike;
  EventDispatcher: new (config: Record<string, unknown>) => EventDispatcherLike;
  LoggerLevel: {
    info: unknown;
  };
};

type ConnectionState = {
  employeeId: string;
  wsClient: WsClientLike | null;
  running: boolean;
  startedAt: string | null;
  lastEventAt: string | null;
  lastError: string | null;
  reconnectAttempts: number;
  credentialsHash: string | null;
};

function normalizeInlineText(text: string): string {
  return text.replace(/\s+/g, ' ').trim();
}

function collectMentions(data: any): any[] {
  const rootMentions = Array.isArray(data?.mentions) ? data.mentions : [];
  const messageMentions = Array.isArray(data?.message?.mentions) ? data.message.mentions : [];
  if (rootMentions.length === 0) return messageMentions;
  if (messageMentions.length === 0) return rootMentions;
  return [...rootMentions, ...messageMentions];
}

function sanitizeMentionTokens(text: string, mentions: any[]): string {
  let next = text;
  for (const mention of mentions) {
    const key = typeof mention?.key === 'string' ? mention.key.trim() : '';
    if (key) {
      next = next.split(key).join(' ');
    }
  }
  // 兜底移除飞书占位 @（例如 @_user_1），避免进入桌面端会话提示词。
  next = next.replace(/@_[A-Za-z0-9_]+/g, ' ');
  return normalizeInlineText(next);
}

function parseText(content: unknown, mentions: any[]): string {
  if (typeof content !== 'string' || !content.trim()) return '';
  const raw = (() => {
    try {
      const obj = JSON.parse(content);
      return typeof obj.text === 'string' ? obj.text : content;
    } catch {
      return content;
    }
  })();
  return sanitizeMentionTokens(raw, mentions);
}

function extractMentionOpenIdFromMentions(mentions: any[]): string {
  for (const mention of mentions) {
    const openId =
      mention?.id?.open_id ||
      mention?.mention_id?.open_id ||
      mention?.open_id ||
      '';
    if (typeof openId === 'string' && openId.trim()) {
      return openId.trim();
    }
  }
  return '';
}

function extractMentionOpenId(data: any): string {
  const mentions = collectMentions(data);
  if (mentions.length > 0) {
    return extractMentionOpenIdFromMentions(mentions);
  }
  try {
    const messageContent = typeof data?.message?.content === 'string' ? data.message.content : '';
    const parsed = JSON.parse(messageContent);
    const contentMentions = Array.isArray(parsed?.mentions) ? parsed.mentions : [];
    return extractMentionOpenIdFromMentions(contentMentions);
  } catch {
    return '';
  }
}

function buildStableEventId(chatId: string, messageId: string): string {
  const chat = chatId.trim();
  const message = messageId.trim();
  if (chat && message) {
    return `${chat}:${message}`;
  }
  if (message) {
    return message;
  }
  if (chat) {
    return `${chat}:${Date.now()}`;
  }
  return `${Date.now()}-${Math.random().toString(16).slice(2)}`;
}

export class FeishuLongConnectionManager {
  private readonly sdk: LarkSdkLike;
  private readonly connections = new Map<string, ConnectionState>();
  private events: FeishuWsEventRecord[] = [];
  private readonly maxEvents = 500;
  private static readonly DEFAULT_EMPLOYEE_ID = 'default';

  constructor(sdk: LarkSdkLike = Lark as unknown as LarkSdkLike) {
    this.sdk = sdk;
  }

  private normalizeEmployeeId(input: string | undefined): string {
    const v = (input || '').trim();
    return v || FeishuLongConnectionManager.DEFAULT_EMPLOYEE_ID;
  }

  private credentialsHash(appId: string, appSecret: string): string {
    return `${appId}::${appSecret}`;
  }

  private getOrCreate(employeeId: string): ConnectionState {
    const existing = this.connections.get(employeeId);
    if (existing) {
      return existing;
    }
    const next: ConnectionState = {
      employeeId,
      wsClient: null,
      running: false,
      startedAt: null,
      lastEventAt: null,
      lastError: null,
      reconnectAttempts: 0,
      credentialsHash: null,
    };
    this.connections.set(employeeId, next);
    return next;
  }

  private pushEvent(rec: FeishuWsEventRecord): void {
    this.events.push(rec);
    if (this.events.length > this.maxEvents) {
      this.events.splice(0, this.events.length - this.maxEvents);
    }
  }

  private stopConnection(employeeId: string): void {
    const state = this.connections.get(employeeId);
    if (!state) {
      return;
    }
    if (state.wsClient) {
      try {
        state.wsClient.stop?.();
      } catch {}
      try {
        state.wsClient.close?.();
      } catch {}
      state.wsClient = null;
    }
    state.running = false;
    state.startedAt = null;
  }

  private startConnection(
    employeeId: string,
    appIdRaw: string | undefined,
    appSecretRaw: string | undefined,
  ): FeishuEmployeeWsStatus {
    const appId = (appIdRaw || (employeeId === FeishuLongConnectionManager.DEFAULT_EMPLOYEE_ID ? process.env.FEISHU_APP_ID : '') || '').trim();
    const appSecret = (appSecretRaw || (employeeId === FeishuLongConnectionManager.DEFAULT_EMPLOYEE_ID ? process.env.FEISHU_APP_SECRET : '') || '').trim();
    if (!appId || !appSecret) {
      throw new Error('Missing Feishu credential: app_id/app_secret');
    }

    const state = this.getOrCreate(employeeId);
    this.stopConnection(employeeId);

    const baseConfig = { appId, appSecret, loggerLevel: this.sdk.LoggerLevel.info };
    const dispatcher = new this.sdk.EventDispatcher({}).register({
      'im.message.receive_v1': async (data: any) => {
        const now = new Date().toISOString();
        const chatId = data?.message?.chat_id || '';
        const messageId = data?.message?.message_id || '';
        const mentions = collectMentions(data);
        const rec: FeishuWsEventRecord = {
          employee_id: employeeId,
          id: buildStableEventId(chatId, messageId),
          event_type: 'im.message.receive_v1',
          chat_id: chatId,
          message_id: messageId,
          text: parseText(data?.message?.content, mentions),
          mention_open_id: extractMentionOpenId(data),
          sender_open_id: data?.sender?.sender_id?.open_id || '',
          received_at: now,
          raw: data,
        };
        state.lastEventAt = now;
        this.pushEvent(rec);
        return {};
      },
    });

    const wsClient = new this.sdk.WSClient(baseConfig);
    wsClient.start({ eventDispatcher: dispatcher });
    state.wsClient = wsClient;
    state.running = true;
    state.startedAt = new Date().toISOString();
    state.lastError = null;
    state.credentialsHash = this.credentialsHash(appId, appSecret);
    return this.statusByEmployee(employeeId);
  }

  private toEmployeeStatus(state: ConnectionState): FeishuEmployeeWsStatus {
    return {
      employee_id: state.employeeId,
      running: state.running,
      started_at: state.startedAt,
      queued_events: this.events.filter((evt) => evt.employee_id === state.employeeId).length,
      last_event_at: state.lastEventAt,
      last_error: state.lastError,
      reconnect_attempts: state.reconnectAttempts,
    };
  }

  private toLegacyStatus(summary: FeishuWsStatusSummary): FeishuWsStatus {
    const firstStarted = summary.items
      .map((item) => item.started_at)
      .find((v) => typeof v === 'string') || null;
    return {
      running: summary.running_count > 0,
      started_at: firstStarted,
      queued_events: summary.queued_events,
    };
  }

  start(input: FeishuWsStartInput): FeishuWsStatus {
    const employeeId = this.normalizeEmployeeId(input.employee_id);
    this.startConnection(employeeId, input.app_id, input.app_secret);
    return this.status();
  }

  reconcile(items: FeishuWsReconcileItem[]): FeishuWsStatusSummary {
    const desired = new Map<string, FeishuWsReconcileItem>();
    for (const item of items) {
      const employeeId = this.normalizeEmployeeId(item.employee_id);
      if (!employeeId) continue;
      desired.set(employeeId, {
        employee_id: employeeId,
        app_id: (item.app_id || '').trim(),
        app_secret: (item.app_secret || '').trim(),
      });
    }

    for (const employeeId of this.connections.keys()) {
      if (!desired.has(employeeId)) {
        this.stopConnection(employeeId);
        this.connections.delete(employeeId);
      }
    }

    for (const [employeeId, item] of desired.entries()) {
      const state = this.getOrCreate(employeeId);
      const nextHash = this.credentialsHash(item.app_id || '', item.app_secret || '');
      const hadConnection = state.running || state.credentialsHash !== null;
      if (!item.app_id || !item.app_secret) {
        this.stopConnection(employeeId);
        state.lastError = 'Missing Feishu credential: app_id/app_secret';
        continue;
      }
      if (state.running && state.credentialsHash === nextHash) {
        continue;
      }
      try {
        this.startConnection(employeeId, item.app_id, item.app_secret);
        if (hadConnection) {
          state.reconnectAttempts += 1;
        }
      } catch (error: any) {
        this.stopConnection(employeeId);
        state.lastError = error?.message || String(error);
      }
    }

    return this.statusAll();
  }

  stop(employeeId?: string): FeishuWsStatus {
    if (employeeId) {
      this.stopConnection(this.normalizeEmployeeId(employeeId));
    } else {
      for (const id of this.connections.keys()) {
        this.stopConnection(id);
      }
    }
    return this.status();
  }

  status(): FeishuWsStatus {
    return this.toLegacyStatus(this.statusAll());
  }

  statusByEmployee(employeeId: string): FeishuEmployeeWsStatus {
    const id = this.normalizeEmployeeId(employeeId);
    const state = this.getOrCreate(id);
    return this.toEmployeeStatus(state);
  }

  statusAll(): FeishuWsStatusSummary {
    const items = Array.from(this.connections.values())
      .sort((a, b) => a.employeeId.localeCompare(b.employeeId))
      .map((state) => this.toEmployeeStatus(state));
    return {
      running: items.some((item) => item.running),
      started_at: items.find((item) => item.started_at)?.started_at || null,
      queued_events: this.events.length,
      running_count: items.filter((item) => item.running).length,
      items,
    };
  }

  drain(limit = 50): FeishuWsEventRecord[] {
    return this.drainAll(limit);
  }

  drainAll(limit = 50, employeeId?: string): FeishuWsEventRecord[] {
    const lim = Math.max(1, Math.min(500, limit));
    if (!employeeId) {
      return this.events.splice(0, lim);
    }
    const target = this.normalizeEmployeeId(employeeId);
    const drained: FeishuWsEventRecord[] = [];
    const remaining: FeishuWsEventRecord[] = [];
    for (const evt of this.events) {
      if (evt.employee_id === target && drained.length < lim) {
        drained.push(evt);
      } else {
        remaining.push(evt);
      }
    }
    this.events = remaining;
    return drained;
  }
}
