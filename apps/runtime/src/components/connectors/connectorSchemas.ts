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
  title: "渠道连接器 / 飞书",
  description: "通过统一连接器模型管理飞书长连接、凭据与重试动作。",
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

export const CONNECTOR_SCHEMAS: Record<string, ConnectorSchema> = {
  feishu: FEISHU_CONNECTOR_SCHEMA,
};

export function getConnectorSchema(connectorId: string): ConnectorSchema {
  return CONNECTOR_SCHEMAS[connectorId] || FEISHU_CONNECTOR_SCHEMA;
}
