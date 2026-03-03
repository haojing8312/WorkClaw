export type RiskLevel = "low" | "medium" | "high";

export interface RiskActionMeta {
  level: RiskLevel;
  title: string;
  summary: string;
  impact?: string;
  irreversible?: boolean;
  confirmLabel?: string;
  cancelLabel?: string;
}

const DEFAULT_LABELS: Record<RiskLevel, { confirm: string; cancel: string }> = {
  low: { confirm: "继续", cancel: "取消" },
  medium: { confirm: "确认", cancel: "取消" },
  high: { confirm: "确认继续", cancel: "取消" },
};

export function normalizeRiskAction(meta: RiskActionMeta): RiskActionMeta {
  const labels = DEFAULT_LABELS[meta.level];
  return {
    ...meta,
    confirmLabel: meta.confirmLabel ?? labels.confirm,
    cancelLabel: meta.cancelLabel ?? labels.cancel,
  };
}

export function riskLevelTone(level: RiskLevel): "info" | "warn" | "danger" {
  if (level === "high") return "danger";
  if (level === "medium") return "warn";
  return "info";
}
