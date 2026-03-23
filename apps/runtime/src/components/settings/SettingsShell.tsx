import type { ReactNode } from "react";

interface SettingsShellProps {
  tabs: ReactNode;
  onClose: () => void;
  children: ReactNode;
}

export function SettingsShell({ tabs, onClose, children }: SettingsShellProps) {
  return (
    <div className="sm-surface-muted flex h-full flex-col overflow-y-auto p-6">
      <div className="flex items-center justify-between mb-6">
        <div className="flex items-center gap-4">{tabs}</div>
        <button onClick={onClose} className="sm-btn sm-btn-ghost h-9 rounded-lg px-4 text-sm">
          返回
        </button>
      </div>

      {children}
    </div>
  );
}
