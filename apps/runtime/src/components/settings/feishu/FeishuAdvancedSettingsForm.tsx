import type { OpenClawPluginFeishuAdvancedSettings } from "../../../types";
import { FeishuAdvancedFieldEditor } from "./FeishuAdvancedFieldEditor";
import {
  FEISHU_ADVANCED_DYNAMIC_AGENT_FIELDS,
  FEISHU_ADVANCED_MESSAGE_FIELDS,
  FEISHU_ADVANCED_ROUTING_FIELDS,
  FEISHU_ADVANCED_RUNTIME_FIELDS,
} from "./feishuAdvancedFieldConfigs";

interface FeishuAdvancedSettingsFormProps {
  feishuAdvancedSettings: OpenClawPluginFeishuAdvancedSettings;
  onUpdateFeishuAdvancedSettings: (patch: Partial<OpenClawPluginFeishuAdvancedSettings>) => void;
  savingFeishuAdvancedSettings: boolean;
  onSaveFeishuAdvancedSettings: () => Promise<void>;
}

export function FeishuAdvancedSettingsForm({
  feishuAdvancedSettings,
  onUpdateFeishuAdvancedSettings,
  savingFeishuAdvancedSettings,
  onSaveFeishuAdvancedSettings,
}: FeishuAdvancedSettingsFormProps) {
  return (
    <details
      className="rounded-lg border border-gray-200 bg-white p-4"
      data-testid="feishu-advanced-settings-form"
    >
      <summary className="cursor-pointer text-sm font-medium text-gray-900">飞书高级配置</summary>
      <div className="mt-2 text-xs text-gray-500">这里可以调整消息格式、接待规则和其他进阶选项。默认设置通常已经够用。</div>
      <div className="mt-3 rounded-lg border border-amber-100 bg-amber-50 px-3 py-2 text-xs text-amber-800">
        建议先完成接入和接待配置，再按需调整这里的参数；不确定时保持默认值通常更稳妥。
      </div>
      <div className="mt-4 space-y-4">
        <details open className="rounded-lg border border-gray-100 bg-gray-50/70 p-3">
          <summary className="cursor-pointer text-sm font-medium text-gray-900">消息与展示</summary>
          <div className="mt-2 text-xs text-gray-500">调整消息输出格式、分块策略和 Markdown 展示方式。</div>
          <div className="mt-4 grid grid-cols-1 gap-4 xl:grid-cols-2">
            {FEISHU_ADVANCED_MESSAGE_FIELDS.map((field) => (
              <FeishuAdvancedFieldEditor
                key={field.key}
                field={field}
                feishuAdvancedSettings={feishuAdvancedSettings}
                onUpdateFeishuAdvancedSettings={onUpdateFeishuAdvancedSettings}
              />
            ))}
          </div>
        </details>

        <details className="rounded-lg border border-gray-100 bg-gray-50/70 p-3">
          <summary className="cursor-pointer text-sm font-medium text-gray-900">群聊与私聊规则</summary>
          <div className="mt-2 text-xs text-gray-500">按群聊或私聊对象自定义启用状态、会话范围和回复规则。</div>
          <div className="mt-4 grid grid-cols-1 gap-4 xl:grid-cols-2">
            {FEISHU_ADVANCED_ROUTING_FIELDS.map((field) => (
              <FeishuAdvancedFieldEditor
                key={field.key}
                field={field}
                feishuAdvancedSettings={feishuAdvancedSettings}
                onUpdateFeishuAdvancedSettings={onUpdateFeishuAdvancedSettings}
              />
            ))}
          </div>
        </details>

        <details className="rounded-lg border border-gray-100 bg-gray-50/70 p-3">
          <summary className="cursor-pointer text-sm font-medium text-gray-900">运行与行为</summary>
          <div className="mt-2 text-xs text-gray-500">调整心跳、媒体限制、超时和插件运行行为。</div>
          <div className="mt-4 grid grid-cols-1 gap-4 xl:grid-cols-2">
            {FEISHU_ADVANCED_RUNTIME_FIELDS.map((field) => (
              <FeishuAdvancedFieldEditor
                key={field.key}
                field={field}
                feishuAdvancedSettings={feishuAdvancedSettings}
                onUpdateFeishuAdvancedSettings={onUpdateFeishuAdvancedSettings}
              />
            ))}
          </div>
          <details className="mt-4 rounded-lg border border-gray-200 bg-white p-3">
            <summary className="cursor-pointer text-sm font-medium text-gray-800">动态 Agent 相关</summary>
            <div className="mt-2 text-xs text-gray-500">只有在需要按飞书会话动态生成 Agent 时才需要调整这里。</div>
            <div className="mt-4 grid grid-cols-1 gap-4 xl:grid-cols-2">
              {FEISHU_ADVANCED_DYNAMIC_AGENT_FIELDS.map((field) => (
                <FeishuAdvancedFieldEditor
                  key={field.key}
                  field={field}
                  feishuAdvancedSettings={feishuAdvancedSettings}
                  onUpdateFeishuAdvancedSettings={onUpdateFeishuAdvancedSettings}
                />
              ))}
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
  );
}
