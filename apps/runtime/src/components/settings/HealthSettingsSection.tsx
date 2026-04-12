import type { ProviderConfig, ProviderHealthInfo, RouteAttemptLog, RouteAttemptStat } from "../../types";

interface HealthSettingsSectionProps {
  allHealthResults: ProviderHealthInfo[];
  filteredRouteLogs: RouteAttemptLog[];
  healthLoading: boolean;
  healthProviderId: string;
  healthResult: ProviderHealthInfo | null;
  inputCls: string;
  providers: ProviderConfig[];
  routeLogsCapabilityFilter: string;
  routeLogsErrorKindFilter: string;
  routeLogsExporting: boolean;
  routeLogsHasMore: boolean;
  routeLogsLoading: boolean;
  routeLogsResultFilter: string;
  routeLogsSessionId: string;
  routeStats: RouteAttemptStat[];
  routeStatsCapability: string;
  routeStatsHours: number;
  routeStatsLoading: boolean;
  onCheckAllProviderHealth: () => void | Promise<void>;
  onCheckProviderHealth: () => void | Promise<void>;
  onCopyRouteLogError: (message: string) => void | Promise<void>;
  onCopyRouteLogSessionId: (sessionId: string) => void | Promise<void>;
  onExportRouteLogsCsv: () => void | Promise<void>;
  onLoadMoreRouteLogs: () => void | Promise<void>;
  onLoadRouteStats: () => void | Promise<void>;
  onRefreshRouteLogs: () => void | Promise<void>;
  onRouteLogsCapabilityFilterChange: (value: string) => void;
  onRouteLogsErrorKindFilterChange: (value: string) => void;
  onRouteLogsResultFilterChange: (value: string) => void;
  onRouteLogsSessionIdChange: (value: string) => void;
  onRouteStatsCapabilityChange: (value: string) => void;
  onRouteStatsHoursChange: (value: number) => void;
  onSelectHealthProvider: (providerId: string) => void;
}

export function HealthSettingsSection({
  allHealthResults,
  filteredRouteLogs,
  healthLoading,
  healthProviderId,
  healthResult,
  inputCls,
  providers,
  routeLogsCapabilityFilter,
  routeLogsErrorKindFilter,
  routeLogsExporting,
  routeLogsHasMore,
  routeLogsLoading,
  routeLogsResultFilter,
  routeLogsSessionId,
  routeStats,
  routeStatsCapability,
  routeStatsHours,
  routeStatsLoading,
  onCheckAllProviderHealth,
  onCheckProviderHealth,
  onCopyRouteLogError,
  onCopyRouteLogSessionId,
  onExportRouteLogsCsv,
  onLoadMoreRouteLogs,
  onLoadRouteStats,
  onRefreshRouteLogs,
  onRouteLogsCapabilityFilterChange,
  onRouteLogsErrorKindFilterChange,
  onRouteLogsResultFilterChange,
  onRouteLogsSessionIdChange,
  onRouteStatsCapabilityChange,
  onRouteStatsHoursChange,
  onSelectHealthProvider,
}: HealthSettingsSectionProps) {
  return (
    <div className="bg-white rounded-lg p-4 space-y-3">
      <div className="text-xs font-medium text-gray-500 mb-2">连接健康检查</div>
      <div>
        <label className="sm-field-label">选择连接</label>
        <select
          className={inputCls}
          value={healthProviderId}
          onChange={(e) => onSelectHealthProvider(e.target.value)}
        >
          <option value="">请选择</option>
          {providers.map((provider) => (
            <option key={provider.id} value={provider.id}>
              {provider.display_name}
            </option>
          ))}
        </select>
      </div>
      <button
        onClick={onCheckProviderHealth}
        disabled={!healthProviderId || healthLoading}
        className="w-full bg-gray-100 hover:bg-gray-200 disabled:opacity-50 text-sm py-1.5 rounded-lg transition-all active:scale-[0.97]"
      >
        {healthLoading ? "检测中..." : "执行健康检查"}
      </button>
      <button
        onClick={onCheckAllProviderHealth}
        disabled={healthLoading}
        className="w-full bg-blue-500 hover:bg-blue-600 disabled:opacity-50 text-white text-sm py-1.5 rounded-lg transition-all active:scale-[0.97]"
      >
        {healthLoading ? "检测中..." : "一键巡检全部连接"}
      </button>
      {healthResult && (
        <div className={"text-xs px-2 py-2 rounded " + (healthResult.ok ? "bg-green-50 text-green-700" : "bg-red-50 text-red-700")}>
          <div>状态: {healthResult.ok ? "正常" : "异常"}</div>
          <div>协议: {healthResult.protocol_type || "-"}</div>
          <div className="break-all">详情: {healthResult.message}</div>
        </div>
      )}
      {allHealthResults.length > 0 && (
        <div className="space-y-2">
          {allHealthResults.map((result, index) => (
            <div key={`${result.provider_id}-${index}`} className={"text-xs px-2 py-2 rounded " + (result.ok ? "bg-green-50 text-green-700" : "bg-red-50 text-red-700")}>
              <div>连接ID: {result.provider_id || "-"}</div>
              <div>状态: {result.ok ? "正常" : "异常"}</div>
              <div>协议: {result.protocol_type || "-"}</div>
              <div className="break-all">详情: {result.message}</div>
            </div>
          ))}
        </div>
      )}
      <div className="pt-2 border-t border-gray-100">
        <div className="mb-3">
          <div className="flex items-center justify-between mb-2">
            <div className="text-xs font-medium text-gray-500">路由统计</div>
            <button
              onClick={onLoadRouteStats}
              disabled={routeStatsLoading}
              className="text-xs text-blue-500 hover:text-blue-600 disabled:opacity-50"
            >
              {routeStatsLoading ? "刷新中..." : "刷新"}
            </button>
          </div>
          <div className="flex gap-2 mb-2">
            <select
              className={inputCls}
              value={String(routeStatsHours)}
              onChange={(e) => onRouteStatsHoursChange(Number(e.target.value || 24))}
            >
              <option value="1">最近 1h</option>
              <option value="24">最近 24h</option>
              <option value="168">最近 7d</option>
            </select>
            <select
              className={inputCls}
              value={routeStatsCapability}
              onChange={(e) => onRouteStatsCapabilityChange(e.target.value)}
            >
              <option value="all">全部能力</option>
              <option value="chat">chat</option>
              <option value="vision">vision</option>
              <option value="image_gen">image_gen</option>
              <option value="audio_stt">audio_stt</option>
              <option value="audio_tts">audio_tts</option>
            </select>
            <button
              onClick={onLoadRouteStats}
              disabled={routeStatsLoading}
              className="bg-gray-100 hover:bg-gray-200 disabled:opacity-50 text-xs px-3 rounded"
            >
              应用
            </button>
          </div>
          {routeStats.length === 0 ? (
            <div className="text-xs text-gray-400">暂无统计数据</div>
          ) : (
            <div className="space-y-1">
              {routeStats.slice(0, 8).map((stat, index) => (
                <div key={`${stat.capability}-${stat.error_kind}-${index}`} className="text-xs bg-gray-50 border border-gray-100 rounded px-2 py-1 text-gray-700">
                  {stat.capability} · {stat.success ? "success" : stat.error_kind || "unknown"} · {stat.count}
                </div>
              ))}
            </div>
          )}
        </div>
        <div className="flex items-center justify-between mb-2">
          <div className="text-xs font-medium text-gray-500">最近路由日志</div>
          <button
            onClick={onRefreshRouteLogs}
            disabled={routeLogsLoading}
            className="text-xs text-blue-500 hover:text-blue-600 disabled:opacity-50"
          >
            {routeLogsLoading ? "刷新中..." : "刷新"}
          </button>
        </div>
        <button
          onClick={onExportRouteLogsCsv}
          disabled={routeLogsExporting}
          className="w-full mb-2 bg-gray-100 hover:bg-gray-200 disabled:opacity-50 text-xs py-1.5 rounded"
        >
          {routeLogsExporting ? "导出中..." : "导出日志 CSV（保存文件并复制到剪贴板）"}
        </button>
        <div className="grid grid-cols-2 gap-2 mb-2">
          <input
            className={inputCls}
            placeholder="按 Session ID 过滤（可选）"
            value={routeLogsSessionId}
            onChange={(e) => onRouteLogsSessionIdChange(e.target.value)}
          />
          <button
            onClick={onRefreshRouteLogs}
            disabled={routeLogsLoading}
            className="bg-gray-100 hover:bg-gray-200 disabled:opacity-50 text-xs py-1.5 rounded"
          >
            应用过滤
          </button>
          <select
            className={inputCls}
            value={routeLogsCapabilityFilter}
            onChange={(e) => onRouteLogsCapabilityFilterChange(e.target.value)}
          >
            <option value="all">能力: 全部</option>
            <option value="chat">chat</option>
            <option value="vision">vision</option>
            <option value="image_gen">image_gen</option>
            <option value="audio_stt">audio_stt</option>
            <option value="audio_tts">audio_tts</option>
          </select>
          <select
            className={inputCls}
            value={routeLogsResultFilter}
            onChange={(e) => onRouteLogsResultFilterChange(e.target.value)}
          >
            <option value="all">结果: 全部</option>
            <option value="success">成功</option>
            <option value="failed">失败</option>
          </select>
          <select
            className={inputCls}
            value={routeLogsErrorKindFilter}
            onChange={(e) => onRouteLogsErrorKindFilterChange(e.target.value)}
          >
            <option value="all">错误类型: 全部</option>
            <option value="auth">auth</option>
            <option value="rate_limit">rate_limit</option>
            <option value="timeout">timeout</option>
            <option value="network">network</option>
            <option value="unknown">unknown</option>
          </select>
        </div>
        {filteredRouteLogs.length === 0 ? (
          <div className="text-xs text-gray-400">暂无路由日志</div>
        ) : (
          <div className="space-y-2 max-h-72 overflow-y-auto pr-1">
            {filteredRouteLogs.map((log, index) => (
              <div key={`${log.created_at}-${index}`} className={"text-xs rounded px-2 py-2 border " + (log.success ? "bg-green-50 border-green-100 text-green-700" : "bg-red-50 border-red-100 text-red-700")}>
                <div>{log.created_at}</div>
                <div>能力: {log.capability} · 协议: {log.api_format}</div>
                <div>模型: {log.model_name}</div>
                <div>尝试: #{log.attempt_index} / 重试: {log.retry_index}</div>
                <div className="flex gap-2 mt-1">
                  <button
                    onClick={() => onRouteLogsSessionIdChange(log.session_id)}
                    className="text-[11px] text-blue-600 hover:text-blue-700"
                  >
                    按此 Session 过滤
                  </button>
                  <button
                    onClick={() => onCopyRouteLogSessionId(log.session_id)}
                    className="text-[11px] text-blue-600 hover:text-blue-700"
                  >
                    复制 Session ID
                  </button>
                  {!log.success && log.error_message && (
                    <button onClick={() => void onCopyRouteLogError(log.error_message)} className="text-[11px] text-blue-600 hover:text-blue-700">
                      复制错误详情
                    </button>
                  )}
                </div>
                <div>结果: {log.success ? "成功" : `失败 (${log.error_kind || "unknown"})`}</div>
                {!log.success && log.error_message && <div className="break-all">错误: {log.error_message}</div>}
              </div>
            ))}
          </div>
        )}
        {routeLogsHasMore && (
          <button
            onClick={onLoadMoreRouteLogs}
            disabled={routeLogsLoading}
            className="w-full mt-2 bg-gray-100 hover:bg-gray-200 disabled:opacity-50 text-xs py-1.5 rounded"
          >
            {routeLogsLoading ? "加载中..." : "加载更多"}
          </button>
        )}
      </div>
    </div>
  );
}
