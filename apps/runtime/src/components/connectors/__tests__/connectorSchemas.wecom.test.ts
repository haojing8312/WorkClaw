import { describe, expect, test } from "vitest";
import { CONNECTOR_SCHEMAS, getConnectorSchema } from "../connectorSchemas";

describe("connectorSchemas wecom", () => {
  test("registers wecom as a connector-backed schema", () => {
    expect(CONNECTOR_SCHEMAS.wecom).toBeDefined();

    const schema = getConnectorSchema("wecom");
    expect(schema.id).toBe("wecom");
    expect(schema.label).toBe("企业微信");
    expect(schema.title).toBe("企业微信连接器");
    expect(schema.description).toBe("接入企业微信消息，交给统一的消息处理规则分发。");
    expect(schema.fields.map((field) => field.key)).toEqual([
      "corpId",
      "agentId",
      "agentSecret",
    ]);
  });
});
