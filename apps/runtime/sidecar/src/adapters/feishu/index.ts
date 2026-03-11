import { FeishuClient, type FeishuSendMessageInput } from "../../feishu.js";
import {
  FeishuLongConnectionManager,
  type FeishuEmployeeWsStatus,
} from "../../feishu_ws.js";
import type {
  AckRequest,
  AdapterConfig,
  AdapterHandle,
  AdapterHealth,
  ChannelAdapter,
  ConnectorDescriptor,
  SendMessageRequest,
  SendMessageResult,
} from "../types.js";
import { classifyConnectorIssue } from "../types.js";
import { normalizeFeishuEvent } from "./normalize.js";

interface FeishuInstanceState {
  employeeId: string;
  appId: string;
  appSecret: string;
}

type FeishuWsManagerLike = Pick<
  FeishuLongConnectionManager,
  "reconcile" | "statusByEmployee" | "drainAll" | "stop"
>;

function toHealth(status: FeishuEmployeeWsStatus, instanceId: string): AdapterHealth {
  return {
    adapter_name: "feishu",
    instance_id: instanceId,
    state: status.running ? "running" : status.last_error ? "error" : "stopped",
    last_ok_at: status.last_event_at || status.started_at,
    last_error: status.last_error,
    reconnect_attempts: status.reconnect_attempts,
    queue_depth: status.queued_events,
    issue: classifyConnectorIssue(status.last_error, status.last_event_at || status.started_at),
  };
}

function toTextContent(text: string): string {
  return JSON.stringify({ text });
}

export class FeishuChannelAdapter implements ChannelAdapter {
  private readonly instances = new Map<string, FeishuInstanceState>();

  constructor(
    private readonly client: Pick<FeishuClient, "sendMessage"> = new FeishuClient(),
    private readonly manager: FeishuWsManagerLike = new FeishuLongConnectionManager(),
  ) {}

  async describe(): Promise<ConnectorDescriptor> {
    return {
      channel: "feishu",
      display_name: "飞书连接器",
      capabilities: ["receive_text", "send_text", "group_route", "direct_route"],
    };
  }

  async start(config: AdapterConfig): Promise<AdapterHandle> {
    const employeeId = String(config.settings.employee_id || config.connector_id).trim();
    const appId = String(config.settings.app_id || "").trim();
    const appSecret = String(config.settings.app_secret || "").trim();
    if (!employeeId || !appId || !appSecret) {
      throw new Error("Missing Feishu adapter settings: employee_id/app_id/app_secret");
    }

    this.manager.reconcile([
      {
        employee_id: employeeId,
        app_id: appId,
        app_secret: appSecret,
      },
    ]);

    const instanceId = `feishu:${config.connector_id}`;
    this.instances.set(instanceId, {
      employeeId,
      appId,
      appSecret,
    });
    return { instance_id: instanceId };
  }

  async stop(instanceId: string): Promise<void> {
    const instance = this.getInstance(instanceId);
    this.manager.stop(instance.employeeId);
    this.instances.delete(instanceId);
  }

  async health(instanceId: string): Promise<AdapterHealth> {
    const instance = this.getInstance(instanceId);
    return toHealth(this.manager.statusByEmployee(instance.employeeId), instanceId);
  }

  async drainEvents(instanceId: string, limit: number) {
    const instance = this.getInstance(instanceId);
    return this.manager
      .drainAll(limit, instance.employeeId)
      .map((event) => normalizeFeishuEvent(event));
  }

  async sendMessage(
    instanceId: string,
    req: SendMessageRequest,
  ): Promise<SendMessageResult> {
    const instance = this.getInstance(instanceId);
    const payload: FeishuSendMessageInput = {
      app_id: instance.appId,
      app_secret: instance.appSecret,
      receive_id: req.reply_target || req.thread_id,
      receive_id_type: "chat_id",
      msg_type: "text",
      content: toTextContent(req.text),
      uuid: instanceId,
    };
    const response = await this.client.sendMessage(payload);
    const data =
      response && typeof response === "object" && "data" in response
        ? (response.data as Record<string, unknown> | undefined)
        : undefined;
    return {
      message_id: typeof data?.message_id === "string" ? data.message_id : "",
      delivered_at: new Date().toISOString(),
      raw_response: response,
    };
  }

  async ack(_instanceId: string, _req: AckRequest): Promise<void> {}

  private getInstance(instanceId: string): FeishuInstanceState {
    const instance = this.instances.get(instanceId);
    if (!instance) {
      throw new Error(`unknown adapter instance: ${instanceId}`);
    }
    return instance;
  }
}
