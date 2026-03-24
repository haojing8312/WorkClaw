export type EmployeeHubTab = "overview" | "employees" | "teams" | "runs" | "settings";

export type EmployeeHubTabNavItem = {
  id: EmployeeHubTab;
  label: string;
};

export interface EmployeeHubTabNavProps {
  tabs: EmployeeHubTabNavItem[];
  activeTab: EmployeeHubTab;
  onTabChange: (tab: EmployeeHubTab) => void;
}

export function EmployeeHubTabNav({ tabs, activeTab, onTabChange }: EmployeeHubTabNavProps) {
  return (
    <div className="rounded-xl border border-[var(--sm-border)] bg-[var(--sm-surface)] p-2 shadow-[var(--sm-shadow-sm)]">
      <div role="tablist" aria-label="智能体员工导航" className="flex flex-wrap gap-2">
        {tabs.map((tab) => {
          const selected = activeTab === tab.id;
          return (
            <button
              key={tab.id}
              id={`employee-hub-tab-${tab.id}`}
              type="button"
              role="tab"
              aria-selected={selected}
              aria-controls={`employee-hub-panel-${tab.id}`}
              tabIndex={selected ? 0 : -1}
              onClick={() => onTabChange(tab.id)}
              className={
                "h-9 px-4 rounded-lg text-sm transition " +
                (selected
                  ? "border-[var(--sm-primary-soft)] bg-[var(--sm-primary-soft)] text-[var(--sm-primary-strong)] shadow-[var(--sm-shadow-sm)]"
                  : "border border-[var(--sm-border)] bg-[var(--sm-surface)] text-[var(--sm-text-muted)] hover:bg-[var(--sm-surface-muted)]")
              }
            >
              {tab.label}
            </button>
          );
        })}
      </div>
    </div>
  );
}
