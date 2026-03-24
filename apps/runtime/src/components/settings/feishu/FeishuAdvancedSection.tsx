import type { OpenClawPluginFeishuAdvancedSettings } from "../../../types";

type FeishuAdvancedFieldConfig = {
  key: keyof OpenClawPluginFeishuAdvancedSettings;
  label: string;
  description: string;
  kind: "input" | "textarea";
  rows?: number;
};

const FEISHU_ADVANCED_MESSAGE_FIELDS: FeishuAdvancedFieldConfig[] = [
  { key: "footer_json", label: "回复页脚 JSON", description: "定义回复尾部展示的状态、耗时等附加信息。", kind: "textarea", rows: 5 },
  { key: "account_overrides_json", label: "账号覆盖 JSON", description: "按账号覆盖消息展示行为，适合多账号接入时做细分调整。", kind: "textarea", rows: 5 },
  { key: "render_mode", label: "渲染模式", description: "控制回复内容的主要渲染方式。", kind: "input" },
  { key: "streaming", label: "流式输出", description: "决定回复是否边生成边发送。", kind: "input" },
  { key: "text_chunk_limit", label: "文本分块上限", description: "单次消息的最大文本块长度。", kind: "input" },
  { key: "chunk_mode", label: "分块模式", description: "控制长消息按什么策略拆分。", kind: "input" },
  { key: "markdown_mode", label: "Markdown 模式", description: "控制 Markdown 内容如何转换给飞书。", kind: "input" },
  { key: "markdown_table_mode", label: "Markdown 表格模式", description: "控制表格内容的展示方式。", kind: "input" },
];

const FEISHU_ADVANCED_ROUTING_FIELDS: FeishuAdvancedFieldConfig[] = [
  { key: "groups_json", label: "群聊高级规则 JSON", description: "按群聊配置启用、提及规则等进阶行为。", kind: "textarea", rows: 8 },
  { key: "dms_json", label: "私聊高级规则 JSON", description: "按私聊对象配置启用状态和系统提示。", kind: "textarea", rows: 8 },
  { key: "reply_in_thread", label: "线程内回复", description: "控制消息是否优先在线程中回复。", kind: "input" },
  { key: "group_session_scope", label: "群聊会话范围", description: "决定群聊里如何划分会话上下文。", kind: "input" },
  { key: "topic_session_mode", label: "话题会话模式", description: "决定是否把话题回复视为独立会话。", kind: "input" },
];

const FEISHU_ADVANCED_RUNTIME_FIELDS: FeishuAdvancedFieldConfig[] = [
  { key: "heartbeat_visibility", label: "心跳可见性", description: "控制连接保活提示是否对外可见。", kind: "input" },
  { key: "heartbeat_interval_ms", label: "心跳间隔毫秒", description: "设置连接保活检测频率。", kind: "input" },
  { key: "media_max_mb", label: "媒体大小上限 MB", description: "限制可处理媒体消息的大小。", kind: "input" },
  { key: "http_timeout_ms", label: "HTTP 超时毫秒", description: "设置外部请求的最大等待时间。", kind: "input" },
  { key: "config_writes", label: "允许插件写回配置", description: "决定插件运行时是否允许自动写回部分配置。", kind: "input" },
];

const FEISHU_ADVANCED_DYNAMIC_AGENT_FIELDS: FeishuAdvancedFieldConfig[] = [
  { key: "dynamic_agent_creation_enabled", label: "动态 Agent 创建", description: "决定是否允许根据飞书会话动态创建 Agent。", kind: "input" },
  { key: "dynamic_agent_creation_workspace_template", label: "动态工作区模板", description: "定义动态创建工作区时使用的路径模板。", kind: "input" },
  { key: "dynamic_agent_creation_agent_dir_template", label: "动态 Agent 目录模板", description: "定义动态 Agent 目录的生成规则。", kind: "input" },
  { key: "dynamic_agent_creation_max_agents", label: "动态 Agent 数量上限", description: "限制动态创建 Agent 的最大数量。", kind: "input" },
];

export interface FeishuAdvancedSectionProps {
  connectionDetailSummary: string;
  feishuAdvancedSettings: OpenClawPluginFeishuAdvancedSettings;
  onUpdateFeishuAdvancedSettings: (patch: Partial<OpenClawPluginFeishuAdvancedSettings>) => void;
  connectionStatusLabel: string;
  pluginVersionLabel: string;
  currentAccountLabel: string;
  pendingPairingCount: number;
  lastEventAtLabel: string;
  recentIssueLabel: string;
  runtimeLogsLabel: string;
  retryingFeishuConnector: boolean;
  savingFeishuAdvancedSettings: boolean;
  onRefreshFeishuSetup: () => Promise<void>;
  onSaveFeishuAdvancedSettings: () => Promise<void>;
  onCopyDiagnostics: () => Promise<void>;
}

function renderFeishuAdvancedField(
  field: FeishuAdvancedFieldConfig,
  feishuAdvancedSettings: OpenClawPluginFeishuAdvancedSettings,
  onUpdateFeishuAdvancedSettings: (patch: Partial<OpenClawPluginFeishuAdvancedSettings>) => void,
) {
  const value = feishuAdvancedSettings[field.key];
  const updateValue = (nextValue: string) => onUpdateFeishuAdvancedSettings({ [field.key]: nextValue });

  return (
    <label key={field.key} className="space-y-1.5">
      <div className="flex items-center justify-between gap-3">
        <div className="text-[11px] font-medium text-gray-700">{field.label}</div>
        <div className="text-[10px] text-gray-400">{field.kind === "textarea" ? "JSON / 模板" : "文本值"}</div>
      </div>
      <div className="text-[11px] leading-5 text-gray-500">{field.description}</div>
      {field.kind === "textarea" ? (
        <textarea
          aria-label={field.label}
          value={value}
          onChange={(event) => updateValue(event.target.value)}
          rows={field.rows ?? 5}
          className="w-full rounded border border-gray-200 bg-gray-50 px-3 py-2 font-mono text-[11px] text-gray-900"
        />
      ) : (
        <input
          aria-label={field.label}
          value={value}
          onChange={(event) => updateValue(event.target.value)}
          className="w-full rounded border border-gray-200 bg-gray-50 px-3 py-2 text-[11px] text-gray-900"
        />
      )}
    </label>
  );
}

export function FeishuAdvancedSection({
  connectionDetailSummary,
  feishuAdvancedSettings,
  onUpdateFeishuAdvancedSettings,
  connectionStatusLabel,
  pluginVersionLabel,
  currentAccountLabel,
  pendingPairingCount,
  lastEventAtLabel,
  recentIssueLabel,
  runtimeLogsLabel,
  retryingFeishuConnector,
  savingFeishuAdvancedSettings,
  onRefreshFeishuSetup,
  onSaveFeishuAdvancedSettings,
  onCopyDiagnostics,
}: FeishuAdvancedSectionProps) {
  return (
    <>
      <details className="rounded-lg border border-gray-200 bg-white p-4">
        <summary className="cursor-pointer text-sm font-medium text-gray-900">连接详情</summary>
        <div className="mt-2 text-xs text-gray-500">这里展示当前连接是否正常、最近一次事件，以及排查问题时最有用的诊断摘要。</div>
        <div className="mt-3 rounded-lg border border-blue-100 bg-blue-50 px-3 py-3 text-sm text-blue-900">{connectionDetailSummary}</div>
        <div className="mt-3 flex flex-wrap gap-2">
          <button
            type="button"
            onClick={() => void onRefreshFeishuSetup()}
            disabled={retryingFeishuConnector}
            className="h-8 px-3 rounded border border-gray-200 bg-white text-xs text-gray-700 hover:bg-gray-50 disabled:bg-gray-100"
          >
            {retryingFeishuConnector ? "检测中..." : "重新检测"}
          </button>
          <button
            type="button"
            onClick={() => void onCopyDiagnostics()}
            className="h-8 px-3 rounded border border-blue-200 bg-white text-xs text-blue-700 hover:bg-blue-50"
          >
            复制诊断摘要
          </button>
        </div>
        <div className="mt-3 grid grid-cols-1 gap-3 md:grid-cols-2 xl:grid-cols-4">
          <div className="rounded border border-gray-100 bg-gray-50 px-3 py-2">
            <div className="text-[11px] text-gray-500">当前状态</div>
            <div className="text-sm font-medium text-gray-900">{connectionStatusLabel}</div>
          </div>
          <div className="rounded border border-gray-100 bg-gray-50 px-3 py-2">
            <div className="text-[11px] text-gray-500">插件版本</div>
            <div className="text-sm font-medium text-gray-900">{pluginVersionLabel}</div>
          </div>
          <div className="rounded border border-gray-100 bg-gray-50 px-3 py-2">
            <div className="text-[11px] text-gray-500">当前接入账号</div>
            <div className="text-sm font-medium text-gray-900">{currentAccountLabel}</div>
          </div>
          <div className="rounded border border-gray-100 bg-gray-50 px-3 py-2">
            <div className="text-[11px] text-gray-500">待完成授权</div>
            <div className="text-sm font-medium text-gray-900">{pendingPairingCount}</div>
          </div>
          <div className="rounded border border-gray-100 bg-gray-50 px-3 py-2 md:col-span-2">
            <div className="text-[11px] text-gray-500">最近一次事件</div>
            <div className="text-sm font-medium text-gray-900">{lastEventAtLabel}</div>
          </div>
          <div className="rounded border border-gray-100 bg-gray-50 px-3 py-2 md:col-span-2">
            <div className="text-[11px] text-gray-500">最近问题</div>
            <div className="text-sm font-medium text-gray-900">{recentIssueLabel}</div>
          </div>
        </div>
        <details className="mt-3 rounded-lg border border-gray-100 bg-gray-50 p-3">
          <summary className="cursor-pointer text-xs font-medium text-gray-700">原始日志（最近 3 条）</summary>
          <div className="mt-2 text-xs text-gray-700 whitespace-pre-wrap break-all">{runtimeLogsLabel}</div>
        </details>
      </details>
      <details className="rounded-lg border border-gray-200 bg-white p-4">
        <summary className="cursor-pointer text-sm font-medium text-gray-900">高级设置</summary>
        <div className="mt-2 text-xs text-gray-500">这里可以调整消息格式、接待规则和其他进阶选项。默认设置通常已经够用。</div>
        <div className="mt-3 rounded-lg border border-amber-100 bg-amber-50 px-3 py-2 text-xs text-amber-800">
          建议先完成接入和接待配置，再按需调整这里的参数；不确定时保持默认值通常更稳妥。
        </div>
        <div className="mt-4 space-y-4">
          <details open className="rounded-lg border border-gray-100 bg-gray-50/70 p-3">
            <summary className="cursor-pointer text-sm font-medium text-gray-900">消息与展示</summary>
            <div className="mt-2 text-xs text-gray-500">调整消息输出格式、分块策略和 Markdown 展示方式。</div>
            <div className="mt-4 grid grid-cols-1 gap-4 xl:grid-cols-2">
              {FEISHU_ADVANCED_MESSAGE_FIELDS.map((field) =>
                renderFeishuAdvancedField(field, feishuAdvancedSettings, onUpdateFeishuAdvancedSettings),
              )}
            </div>
          </details>

          <details className="rounded-lg border border-gray-100 bg-gray-50/70 p-3">
            <summary className="cursor-pointer text-sm font-medium text-gray-900">群聊与私聊规则</summary>
            <div className="mt-2 text-xs text-gray-500">按群聊或私聊对象自定义启用状态、会话范围和回复规则。</div>
            <div className="mt-4 grid grid-cols-1 gap-4 xl:grid-cols-2">
              {FEISHU_ADVANCED_ROUTING_FIELDS.map((field) =>
                renderFeishuAdvancedField(field, feishuAdvancedSettings, onUpdateFeishuAdvancedSettings),
              )}
            </div>
          </details>

          <details className="rounded-lg border border-gray-100 bg-gray-50/70 p-3">
            <summary className="cursor-pointer text-sm font-medium text-gray-900">运行与行为</summary>
            <div className="mt-2 text-xs text-gray-500">调整心跳、媒体限制、超时和插件运行行为。</div>
            <div className="mt-4 grid grid-cols-1 gap-4 xl:grid-cols-2">
              {FEISHU_ADVANCED_RUNTIME_FIELDS.map((field) =>
                renderFeishuAdvancedField(field, feishuAdvancedSettings, onUpdateFeishuAdvancedSettings),
              )}
            </div>
            <details className="mt-4 rounded-lg border border-gray-200 bg-white p-3">
              <summary className="cursor-pointer text-sm font-medium text-gray-800">动态 Agent 相关</summary>
              <div className="mt-2 text-xs text-gray-500">只有在需要按飞书会话动态生成 Agent 时才需要调整这里。</div>
              <div className="mt-4 grid grid-cols-1 gap-4 xl:grid-cols-2">
                {FEISHU_ADVANCED_DYNAMIC_AGENT_FIELDS.map((field) =>
                  renderFeishuAdvancedField(field, feishuAdvancedSettings, onUpdateFeishuAdvancedSettings),
                )}
              </div>
            </details>
          </details>

          <div className="flex justify-end">
            <button
              type="button"
              onClick={() => void onSaveFeishuAdvancedSettings()}
              disabled={savingFeishuAdvancedSettings}
              className="sm-btn sm-btn-primary h-9 rounded-lg px-4 text-sm disabled:opacity-60"
            >
              {savingFeishuAdvancedSettings ? "保存中..." : "保存高级配置"}
            </button>
          </div>
        </div>
      </details>
    </>
  );
}
