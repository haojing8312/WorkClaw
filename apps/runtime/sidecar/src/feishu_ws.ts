import * as Lark from '@larksuiteoapi/node-sdk';

export interface FeishuWsStartInput {
  app_id?: string;
  app_secret?: string;
}

export interface FeishuWsEventRecord {
  id: string;
  event_type: string;
  chat_id: string;
  message_id: string;
  text: string;
  sender_open_id: string;
  received_at: string;
  raw: unknown;
}

export interface FeishuWsStatus {
  running: boolean;
  started_at: string | null;
  queued_events: number;
}

function parseText(content: unknown): string {
  if (typeof content !== 'string' || !content.trim()) return '';
  try {
    const obj = JSON.parse(content);
    return typeof obj.text === 'string' ? obj.text : content;
  } catch {
    return content;
  }
}

export class FeishuLongConnectionManager {
  private wsClient: any | null = null;
  private running = false;
  private startedAt: string | null = null;
  private events: FeishuWsEventRecord[] = [];
  private readonly maxEvents = 500;

  start(input: FeishuWsStartInput): FeishuWsStatus {
    const appId = (input.app_id || process.env.FEISHU_APP_ID || '').trim();
    const appSecret = (input.app_secret || process.env.FEISHU_APP_SECRET || '').trim();
    if (!appId || !appSecret) {
      throw new Error('Missing Feishu credential: app_id/app_secret');
    }

    this.stop();

    const baseConfig = { appId, appSecret, loggerLevel: Lark.LoggerLevel.info };
    const dispatcher = new Lark.EventDispatcher({}).register({
      'im.message.receive_v1': async (data: any) => {
        const rec: FeishuWsEventRecord = {
          id: `${Date.now()}-${Math.random().toString(16).slice(2)}`,
          event_type: 'im.message.receive_v1',
          chat_id: data?.message?.chat_id || '',
          message_id: data?.message?.message_id || '',
          text: parseText(data?.message?.content),
          sender_open_id: data?.sender?.sender_id?.open_id || '',
          received_at: new Date().toISOString(),
          raw: data,
        };
        this.events.push(rec);
        if (this.events.length > this.maxEvents) {
          this.events.splice(0, this.events.length - this.maxEvents);
        }
        return {};
      },
    });

    this.wsClient = new Lark.WSClient(baseConfig);
    this.wsClient.start({ eventDispatcher: dispatcher });
    this.running = true;
    this.startedAt = new Date().toISOString();
    return this.status();
  }

  stop(): FeishuWsStatus {
    if (this.wsClient) {
      try {
        this.wsClient.stop?.();
      } catch {}
      try {
        this.wsClient.close?.();
      } catch {}
      this.wsClient = null;
    }
    this.running = false;
    this.startedAt = null;
    return this.status();
  }

  status(): FeishuWsStatus {
    return {
      running: this.running,
      started_at: this.startedAt,
      queued_events: this.events.length,
    };
  }

  drain(limit = 50): FeishuWsEventRecord[] {
    const lim = Math.max(1, Math.min(500, limit));
    return this.events.splice(0, lim);
  }
}

