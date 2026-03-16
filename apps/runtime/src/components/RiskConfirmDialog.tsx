import { RiskActionMeta, RiskLevel, normalizeRiskAction } from "./risk-action";

interface RiskConfirmDialogProps {
  open: boolean;
  level: RiskLevel;
  title: string;
  summary: string;
  impact?: string;
  note?: string;
  irreversible?: boolean;
  confirmLabel?: string;
  secondaryActionLabel?: string;
  cancelLabel?: string;
  loading: boolean;
  onConfirm: () => void;
  onSecondaryAction?: () => void;
  onCancel: () => void;
}

function getTitleClass(level: RiskLevel): string {
  if (level === "high") return "text-[var(--sm-danger)]";
  if (level === "medium") return "text-[var(--sm-warn)]";
  return "text-[var(--sm-primary-strong)]";
}

function getBadgeClass(level: RiskLevel): string {
  if (level === "high") return "sm-badge-danger";
  if (level === "medium") return "sm-badge-warn";
  return "sm-badge-info";
}

function getLevelLabel(level: RiskLevel): string {
  if (level === "high") return "高风险";
  if (level === "medium") return "中风险";
  return "低风险";
}

export function RiskConfirmDialog(props: RiskConfirmDialogProps) {
  if (!props.open) return null;

  const meta: RiskActionMeta = normalizeRiskAction({
    level: props.level,
    title: props.title,
    summary: props.summary,
    impact: props.impact,
    irreversible: props.irreversible,
    confirmLabel: props.confirmLabel,
    cancelLabel: props.cancelLabel,
  });

  return (
    <div className="fixed inset-0 z-50 flex items-center justify-center bg-black/40 px-4" role="dialog" aria-modal="true">
      <div className="sm-panel w-full max-w-lg p-5">
        <div className="flex items-center justify-between gap-2">
          <h3 className={`text-base font-semibold ${getTitleClass(meta.level)}`}>{meta.title}</h3>
          <span className={getBadgeClass(meta.level)}>{getLevelLabel(meta.level)}</span>
        </div>

        <p className="mt-2 text-sm sm-text-primary">{meta.summary}</p>

        {meta.impact && <p className="mt-2 text-xs sm-text-muted">{meta.impact}</p>}

        {props.note && <p className="mt-2 text-xs sm-text-primary">{props.note}</p>}

        {meta.irreversible && (
          <p className="mt-3 text-xs sm-text-danger">该操作不可逆，请确认后再继续。</p>
        )}

        <div className="mt-5 flex items-center justify-end gap-2">
          <button
            type="button"
            disabled={props.loading}
            onClick={props.onCancel}
            className="sm-btn sm-btn-secondary h-9 px-4 text-sm disabled:opacity-60"
          >
            {meta.cancelLabel}
          </button>
          {props.secondaryActionLabel && props.onSecondaryAction && (
            <button
              type="button"
              disabled={props.loading}
              onClick={props.onSecondaryAction}
              className="sm-btn sm-btn-secondary h-9 px-4 text-sm disabled:opacity-60"
            >
              {props.secondaryActionLabel}
            </button>
          )}
          <button
            type="button"
            disabled={props.loading}
            onClick={props.onConfirm}
            className={`sm-btn h-9 px-4 text-sm text-white disabled:opacity-60 ${
              meta.level === "high" ? "bg-[var(--sm-danger)] hover:bg-red-700" : "sm-btn-primary"
            }`}
          >
            {props.loading ? "处理中..." : meta.confirmLabel}
          </button>
        </div>
      </div>
    </div>
  );
}
