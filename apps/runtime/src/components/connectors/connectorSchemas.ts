export type ConnectorFieldType = "text" | "password";

export interface ConnectorFieldSchema {
  key: string;
  label: string;
  placeholder: string;
  type?: ConnectorFieldType;
  multiline?: boolean;
  helperText?: string;
}

export interface ConnectorSchema {
  id: string;
  label: string;
  title: string;
  description: string;
  saveLabel: string;
  retryLabel: string;
  fields: ConnectorFieldSchema[];
}

export const FEISHU_CONNECTOR_SCHEMA: ConnectorSchema = {
  id: "feishu",
  label: "飞书",
  title: "飞书连接器",
  description: "接入飞书消息，交给统一的消息处理规则分发。",
  saveLabel: "保存连接器配置",
  retryLabel: "重试连接",
  fields: [
    {
      key: "openId",
      label: "机器人 open_id",
      placeholder: "飞书机器人 open_id（可空，仅用于飞书@精准路由）",
      helperText: "可留空；用于飞书 @ 精准路由。",
    },
    {
      key: "appId",
      label: "App ID",
      placeholder: "机器人 App ID",
    },
    {
      key: "appSecret",
      label: "App Secret",
      placeholder: "机器人 App Secret",
      type: "password",
    },
  ],
};

export const WECOM_CONNECTOR_SCHEMA: ConnectorSchema = {
  id: "wecom",
  label: "企业微信",
  title: "企业微信连接器",
  description: "接入企业微信消息，交给统一的消息处理规则分发。",
  saveLabel: "保存企业微信连接器",
  retryLabel: "重试连接",
  fields: [
    {
      key: "corpId",
      label: "Corp ID",
      placeholder: "企业微信 Corp ID",
    },
    {
      key: "agentId",
      label: "Agent ID",
      placeholder: "企业应用 Agent ID",
    },
    {
      key: "agentSecret",
      label: "Agent Secret",
      placeholder: "企业应用 Secret",
      type: "password",
    },
  ],
};

export const CONNECTOR_SCHEMAS: Record<string, ConnectorSchema> = {
  feishu: FEISHU_CONNECTOR_SCHEMA,
  wecom: WECOM_CONNECTOR_SCHEMA,
};

export function getConnectorSchema(connectorId: string): ConnectorSchema {
  return CONNECTOR_SCHEMAS[connectorId] || FEISHU_CONNECTOR_SCHEMA;
}
