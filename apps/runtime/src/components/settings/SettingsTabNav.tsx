export type SettingsTabName =
  | "models"
  | "desktop"
  | "capabilities"
  | "health"
  | "mcp"
  | "search"
  | "routing"
  | "feishu";

interface SettingsTabNavProps {
  activeTab: SettingsTabName;
  onSelectTab: (tab: SettingsTabName) => void;
  showCapabilityRoutingSettings: boolean;
  showHealthSettings: boolean;
  showMcpSettings: boolean;
  showAutoRoutingSettings: boolean;
}

function getTabButtonClass(activeTab: SettingsTabName, tab: SettingsTabName) {
  return (
    "sm-btn h-8 px-2 rounded-none border-b-2 text-sm font-medium transition-colors " +
    (activeTab === tab
      ? "text-[var(--sm-primary-strong)] border-[var(--sm-primary)]"
      : "sm-text-muted border-transparent hover:text-[var(--sm-text)]")
  );
}

export function SettingsTabNav({
  activeTab,
  onSelectTab,
  showCapabilityRoutingSettings,
  showHealthSettings,
  showMcpSettings,
  showAutoRoutingSettings,
}: SettingsTabNavProps) {
  return (
    <>
      <button onClick={() => onSelectTab("models")} className={getTabButtonClass(activeTab, "models")}>
        模型连接
      </button>
      <button onClick={() => onSelectTab("desktop")} className={getTabButtonClass(activeTab, "desktop")}>
        桌面 / 系统
      </button>
      {showCapabilityRoutingSettings && (
        <button
          onClick={() => onSelectTab("capabilities")}
          className={getTabButtonClass(activeTab, "capabilities")}
        >
          能力路由
        </button>
      )}
      {showHealthSettings && (
        <button onClick={() => onSelectTab("health")} className={getTabButtonClass(activeTab, "health")}>
          健康检查
        </button>
      )}
      {showMcpSettings && (
        <button onClick={() => onSelectTab("mcp")} className={getTabButtonClass(activeTab, "mcp")}>
          MCP 服务器
        </button>
      )}
      <button onClick={() => onSelectTab("search")} className={getTabButtonClass(activeTab, "search")}>
        搜索引擎
      </button>
      <button onClick={() => onSelectTab("feishu")} className={getTabButtonClass(activeTab, "feishu")}>
        渠道连接器
      </button>
      {showAutoRoutingSettings && (
        <button onClick={() => onSelectTab("routing")} className={getTabButtonClass(activeTab, "routing")}>
          自动路由
        </button>
      )}
    </>
  );
}
