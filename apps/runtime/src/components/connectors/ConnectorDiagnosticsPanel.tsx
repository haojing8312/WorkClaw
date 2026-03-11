import { useState } from "react";
import type { ChannelConnectorDiagnostics } from "../../types";

interface Props {
  title?: string;
  diagnostics: ChannelConnectorDiagnostics;
}

function statusLabel(status: string) {
  switch (status) {
    case "connected":
      return "已连接";
    case "degraded":
      return "降级运行";
    case "authentication_error":
      return "权限异常";
    case "connection_error":
      return "连接异常";
    case "needs_configuration":
      return "待配置";
    case "stopped":
      return "已停止";
    default:
      return status || "未知状态";
  }
}

export function ConnectorDiagnosticsPanel({ title = "连接器诊断", diagnostics }: Props) {
  const [showTechnicalDetails, setShowTechnicalDetails] = useState(false);
  const issueSummary =
    diagnostics.health.issue?.user_message ||
    (diagnostics.health.last_error ? "连接异常" : "运行正常");

  return (
    <div
      className="rounded-lg border border-gray-200 bg-white p-3 space-y-3"
      data-testid={`connector-diagnostics-panel-${diagnostics.connector.channel}`}
    >
      <div className="space-y-1">
        <div className="text-xs font-medium text-gray-700">{title}</div>
        <div className="text-sm font-medium text-gray-900">{diagnostics.connector.display_name}</div>
        <div className="text-[11px] text-gray-500">
          {`当前状态：${statusLabel(diagnostics.status)} · 最近问题：${issueSummary}`}
        </div>
      </div>

      <div className="flex flex-wrap gap-2">
        {diagnostics.connector.capabilities.map((capability) => (
          <span
            key={capability}
            className="inline-flex items-center rounded-full bg-gray-100 px-2 py-1 text-[11px] text-gray-700"
          >
            {capability}
          </span>
        ))}
      </div>

      <div className="grid grid-cols-2 md:grid-cols-4 gap-2">
        <div className="rounded border border-gray-100 bg-gray-50 px-2 py-1.5">
          <div className="text-[11px] text-gray-500">连接标识</div>
          <div className="text-xs text-gray-700">{diagnostics.health.instance_id}</div>
        </div>
        <div className="rounded border border-gray-100 bg-gray-50 px-2 py-1.5">
          <div className="text-[11px] text-gray-500">待处理消息</div>
          <div className="text-xs text-gray-700">{diagnostics.health.queue_depth}</div>
        </div>
        <div className="rounded border border-gray-100 bg-gray-50 px-2 py-1.5">
          <div className="text-[11px] text-gray-500">保留事件</div>
          <div className="text-xs text-gray-700">{diagnostics.replay.retained_events}</div>
        </div>
        <div className="rounded border border-gray-100 bg-gray-50 px-2 py-1.5">
          <div className="text-[11px] text-gray-500">已确认事件</div>
          <div className="text-xs text-gray-700">{diagnostics.replay.acked_events}</div>
        </div>
      </div>

      {diagnostics.health.last_error && (
        <div className="space-y-1">
          <button
            type="button"
            onClick={() => setShowTechnicalDetails((value) => !value)}
            className="text-[11px] text-blue-600 hover:text-blue-700"
          >
            {showTechnicalDetails ? "收起技术详情" : "查看技术详情"}
          </button>
          {showTechnicalDetails && (
            <div className="rounded border border-red-100 bg-red-50 px-2 py-2 text-xs text-red-700">
              {`原始错误：${diagnostics.health.issue?.technical_message || diagnostics.health.last_error}`}
            </div>
          )}
        </div>
      )}
    </div>
  );
}
