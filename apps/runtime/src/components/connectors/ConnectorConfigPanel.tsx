import type { ConnectorSchema } from "./connectorSchemas";
import { useState } from "react";

interface Props {
  schema: ConnectorSchema;
  status: {
    dotClass: string;
    label: string;
    detail: string;
    error: string;
  };
  values: Record<string, string>;
  saving: boolean;
  retrying: boolean;
  diagnostics?: Array<{ label: string; value: string }>;
  onChange: (key: string, value: string) => void;
  onSave: () => void | Promise<void>;
  onRetry: () => void | Promise<void>;
}

export function ConnectorConfigPanel({
  schema,
  status,
  values,
  saving,
  retrying,
  diagnostics = [],
  onChange,
  onSave,
  onRetry,
}: Props) {
  const [showTechnicalDetails, setShowTechnicalDetails] = useState(false);

  return (
    <div className="rounded-lg border border-gray-200 p-3 space-y-2" data-testid={`connector-panel-${schema.id}`}>
      <div className="text-xs font-medium text-gray-700">{schema.title}</div>
      <div className="text-[11px] text-gray-500">{schema.description}</div>
      <div className="flex items-center gap-2">
        <span className={`inline-block h-2.5 w-2.5 rounded-full ${status.dotClass}`} />
        <span className="text-xs text-gray-900">{status.label}</span>
      </div>
      <div className="text-[11px] text-gray-500">{status.detail}</div>
      {status.error && (
        <div className="space-y-1">
          <button
            type="button"
            onClick={() => setShowTechnicalDetails((value) => !value)}
            className="text-[11px] text-blue-600 hover:text-blue-700"
          >
            {showTechnicalDetails ? "收起技术详情" : "查看技术详情"}
          </button>
          {showTechnicalDetails && <div className="text-xs text-red-600">{`原始错误：${status.error}`}</div>}
        </div>
      )}
      {diagnostics.length > 0 && (
        <div className="grid grid-cols-1 md:grid-cols-3 gap-2" data-testid={`connector-diagnostics-${schema.id}`}>
          {diagnostics.map((item) => (
            <div key={item.label} className="rounded border border-gray-100 bg-gray-50 px-2 py-1.5">
              <div className="text-[11px] text-gray-500">{item.label}</div>
              <div className="text-xs text-gray-700">{item.value}</div>
            </div>
          ))}
        </div>
      )}

      <div className="grid grid-cols-1 md:grid-cols-2 gap-2">
        {schema.fields.map((field) => (
          <div
            key={field.key}
            className={field.key === "openId" ? "md:col-span-2" : undefined}
            data-testid={`connector-field-${schema.id}-${field.key}`}
          >
            {field.multiline ? (
              <textarea
                className="w-full border border-gray-200 rounded px-2 py-1.5 text-sm"
                placeholder={field.placeholder}
                value={values[field.key] || ""}
                onChange={(e) => onChange(field.key, e.target.value)}
              />
            ) : (
              <input
                className="w-full border border-gray-200 rounded px-2 py-1.5 text-sm"
                type={field.type === "password" ? "password" : "text"}
                placeholder={field.placeholder}
                value={values[field.key] || ""}
                onChange={(e) => onChange(field.key, e.target.value)}
              />
            )}
            {field.helperText && <div className="mt-1 text-[11px] text-gray-500">{field.helperText}</div>}
          </div>
        ))}
      </div>

      <div className="flex items-center gap-2">
        <button
          type="button"
          onClick={() => void onSave()}
          disabled={saving}
          className="h-8 px-3 rounded bg-blue-500 hover:bg-blue-600 disabled:bg-blue-300 text-white text-xs"
        >
          {saving ? "保存中..." : schema.saveLabel}
        </button>
        <button
          type="button"
          onClick={() => void onRetry()}
          disabled={retrying}
          className="h-8 px-3 rounded border border-blue-200 hover:bg-blue-50 disabled:bg-gray-100 text-blue-700 text-xs"
        >
          {retrying ? "重试中..." : schema.retryLabel}
        </button>
      </div>
    </div>
  );
}
