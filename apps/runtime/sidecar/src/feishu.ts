import lark from '@larksuiteoapi/node-sdk';

export interface FeishuSendMessageInput {
  app_id?: string;
  app_secret?: string;
  receive_id: string;
  receive_id_type?: 'chat_id' | 'open_id' | 'user_id' | 'union_id' | 'email';
  msg_type: 'text' | 'post' | 'interactive' | 'image' | 'file' | string;
  content: string;
  uuid?: string;
}

export interface FeishuListChatsInput {
  app_id?: string;
  app_secret?: string;
  page_size?: number;
  page_token?: string;
  user_id_type?: 'open_id' | 'user_id' | 'union_id';
}

export interface FeishuChatItem {
  chat_id: string;
  name: string;
  description?: string;
  owner_id?: string;
}

export interface FeishuListChatsResult {
  items: FeishuChatItem[];
  has_more: boolean;
  page_token?: string;
}

interface FeishuApiResult {
  code?: number;
  msg?: string;
  data?: unknown;
}

export class FeishuClient {
  private client: any | null = null;
  private clientKey = '';

  private resolveCredential(input: FeishuSendMessageInput): { appId: string; appSecret: string } {
    const appId = (input.app_id || process.env.FEISHU_APP_ID || '').trim();
    const appSecret = (input.app_secret || process.env.FEISHU_APP_SECRET || '').trim();
    if (!appId || !appSecret) {
      throw new Error('Missing Feishu credential: app_id/app_secret');
    }
    return { appId, appSecret };
  }

  private getClient(input: FeishuSendMessageInput): any {
    const { appId, appSecret } = this.resolveCredential(input);
    const nextKey = `${appId}:${appSecret}`;
    if (!this.client || this.clientKey !== nextKey) {
      this.client = new lark.Client({ appId, appSecret });
      this.clientKey = nextKey;
    }
    return this.client;
  }

  async sendMessage(input: FeishuSendMessageInput): Promise<FeishuApiResult> {
    const client = this.getClient(input);
    const receiveIdType = input.receive_id_type || 'chat_id';
    const resp = await client.im.v1.message.create({
      params: { receive_id_type: receiveIdType },
      data: {
        receive_id: input.receive_id,
        msg_type: input.msg_type,
        content: input.content,
        uuid: input.uuid,
      },
    });
    return resp as FeishuApiResult;
  }

  async listChats(input: FeishuListChatsInput): Promise<FeishuListChatsResult> {
    const client = this.getClient(input as FeishuSendMessageInput);
    const pageSize = Math.max(1, Math.min(100, Number(input.page_size || 20)));
    const resp = await client.im.v1.chat.list({
      params: {
        user_id_type: input.user_id_type || 'open_id',
        page_size: pageSize,
        page_token: input.page_token,
      },
    });
    const data = (resp?.data || {}) as any;
    const rawItems = Array.isArray(data?.items) ? data.items : [];
    return {
      items: rawItems.map((it: any) => ({
        chat_id: it?.chat_id || '',
        name: it?.name || '',
        description: it?.description || undefined,
        owner_id: it?.owner_id || undefined,
      })),
      has_more: Boolean(data?.has_more),
      page_token: typeof data?.page_token === 'string' ? data.page_token : undefined,
    };
  }
}
