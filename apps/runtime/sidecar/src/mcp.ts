import { Client } from '@modelcontextprotocol/sdk/client/index.js';
import { StdioClientTransport } from '@modelcontextprotocol/sdk/client/stdio.js';

interface MCPServerConfig {
  command: string;
  args?: string[];
  env?: Record<string, string>;
}

export class MCPManager {
  private servers: Map<
    string,
    { client: Client; transport: StdioClientTransport }
  > = new Map();

  async addServer(name: string, config: MCPServerConfig) {
    const transport = new StdioClientTransport({
      command: config.command,
      args: config.args || [],
      env: { ...process.env as Record<string, string>, ...config.env },
    });

    const client = new Client(
      {
        name: 'workclaw-runtime',
        version: '1.0.0',
      },
      {
        capabilities: {},
      }
    );

    await client.connect(transport);
    this.servers.set(name, { client, transport });
  }

  listServers(): string[] {
    return Array.from(this.servers.keys());
  }

  async callTool(
    serverName: string,
    toolName: string,
    args: Record<string, unknown>
  ): Promise<unknown> {
    const server = this.servers.get(serverName);
    if (!server) {
      throw new Error(`MCP 服务器 ${serverName} 不存在`);
    }

    const result = await server.client.callTool({
      name: toolName,
      arguments: args,
    });

    return result;
  }

  async listTools(serverName: string): Promise<unknown[]> {
    const server = this.servers.get(serverName);
    if (!server) {
      throw new Error(`MCP 服务器 ${serverName} 不存在`);
    }

    const response = await server.client.listTools();
    return response.tools;
  }

  async closeAll() {
    for (const [, { client, transport }] of this.servers.entries()) {
      await client.close();
      await transport.close();
    }
    this.servers.clear();
  }
}
