import type {
  FeishuGatewaySettings,
  FeishuPairingRequestRecord,
  FeishuPluginEnvironmentStatus,
  FeishuSetupProgress,
  OpenClawLarkInstallerMode,
  OpenClawLarkInstallerSessionStatus,
  OpenClawPluginFeishuCredentialProbeResult,
  OpenClawPluginFeishuRuntimeStatus,
} from "../../../types";

type FeishuAuthorizationAction = {
  label: string;
  busyLabel: string;
};

type FeishuRoutingStatus = {
  label: string;
  description: string;
  actionLabel: string;
};

export interface FeishuAdvancedConsoleSectionProps {
  onOpenEmployees?: () => void;
  feishuConnectorSettings: FeishuGatewaySettings;
  onUpdateFeishuConnectorSettings: (patch: Partial<FeishuGatewaySettings>) => void;
  feishuEnvironmentStatus: FeishuPluginEnvironmentStatus | null;
  feishuSetupProgress: FeishuSetupProgress | null;
  officialFeishuRuntimeStatus: OpenClawPluginFeishuRuntimeStatus | null;
  feishuCredentialProbe: OpenClawPluginFeishuCredentialProbeResult | null;
  validatingFeishuCredentials: boolean;
  savingFeishuConnector: boolean;
  retryingFeishuConnector: boolean;
  installingOfficialFeishuPlugin: boolean;
  feishuInstallerSession: OpenClawLarkInstallerSessionStatus;
  feishuInstallerInput: string;
  onUpdateFeishuInstallerInput: (value: string) => void;
  feishuInstallerBusy: boolean;
  feishuInstallerStartingMode: OpenClawLarkInstallerMode | null;
  feishuPairingActionLoading: "approve" | "deny" | null;
  pendingFeishuPairingCount: number;
  pendingFeishuPairingRequest: FeishuPairingRequestRecord | null;
  feishuOnboardingEffectiveBranch: "existing_robot" | "create_robot" | null;
  feishuAuthorizationInlineError: string | null;
  feishuOnboardingHeaderStep: string;
  feishuInstallerDisplayMode: OpenClawLarkInstallerMode | null;
  feishuInstallerStartupHint: string | null;
  feishuAuthorizationAction: FeishuAuthorizationAction;
  feishuRoutingStatus: FeishuRoutingStatus;
  getFeishuEnvironmentLabel: (ready: boolean, fallback: string) => string;
  formatCompactDateTime: (value: string | null | undefined) => string;
  handleValidateFeishuCredentials: () => Promise<void>;
  handleSaveFeishuConnector: () => Promise<void>;
  handleInstallAndStartFeishuConnector: () => Promise<void>;
  handleRefreshFeishuSetup: () => Promise<void>;
  handleResolveFeishuPairingRequest: (requestId: string, action: "approve" | "deny") => Promise<void>;
  handleStartFeishuInstaller: (mode: "create" | "link") => Promise<void>;
  handleStopFeishuInstallerSession: () => Promise<void>;
  handleSendFeishuInstallerInput: () => Promise<void>;
}

export function FeishuAdvancedConsoleSection({
  onOpenEmployees,
  feishuConnectorSettings,
  onUpdateFeishuConnectorSettings,
  feishuEnvironmentStatus,
  feishuSetupProgress,
  officialFeishuRuntimeStatus,
  feishuCredentialProbe,
  validatingFeishuCredentials,
  savingFeishuConnector,
  retryingFeishuConnector,
  installingOfficialFeishuPlugin,
  feishuInstallerSession,
  feishuInstallerInput,
  onUpdateFeishuInstallerInput,
  feishuInstallerBusy,
  feishuInstallerStartingMode,
  feishuPairingActionLoading,
  pendingFeishuPairingCount,
  pendingFeishuPairingRequest,
  feishuOnboardingEffectiveBranch,
  feishuAuthorizationInlineError,
  feishuOnboardingHeaderStep,
  feishuInstallerDisplayMode,
  feishuInstallerStartupHint,
  feishuAuthorizationAction,
  feishuRoutingStatus,
  getFeishuEnvironmentLabel,
  formatCompactDateTime,
  handleValidateFeishuCredentials,
  handleSaveFeishuConnector,
  handleInstallAndStartFeishuConnector,
  handleRefreshFeishuSetup,
  handleResolveFeishuPairingRequest,
  handleStartFeishuInstaller,
  handleStopFeishuInstallerSession,
  handleSendFeishuInstallerInput,
}: FeishuAdvancedConsoleSectionProps) {
  return (
    <details className="rounded-lg border border-gray-200 bg-white p-4">
      <summary className="cursor-pointer text-sm font-medium text-gray-900">高级设置与控制台</summary>
      <div className="mt-4 space-y-3">
        <div className="rounded-lg border border-gray-200 bg-white p-4 space-y-3">
          <div>
            <div className="text-sm font-medium text-gray-900">检查运行环境</div>
            <div className="text-xs text-gray-500 mt-1">不内置运行环境；如果电脑未安装 Node.js，系统会在这里提示你先完成安装。</div>
          </div>
          <div className="grid grid-cols-1 gap-3 md:grid-cols-3">
            <div className="rounded border border-gray-100 bg-gray-50 px-3 py-3">
              <div className="text-[11px] text-gray-500">Node.js</div>
              <div className="mt-1 text-sm font-medium text-gray-900">
                {getFeishuEnvironmentLabel(Boolean(feishuEnvironmentStatus?.node_available), "未检测到")}
              </div>
              <div className="mt-1 text-[11px] text-gray-500">{feishuEnvironmentStatus?.node_version || "请安装 Node.js LTS"}</div>
            </div>
            <div className="rounded border border-gray-100 bg-gray-50 px-3 py-3">
              <div className="text-[11px] text-gray-500">npm</div>
              <div className="mt-1 text-sm font-medium text-gray-900">
                {getFeishuEnvironmentLabel(Boolean(feishuEnvironmentStatus?.npm_available), "未检测到")}
              </div>
              <div className="mt-1 text-[11px] text-gray-500">{feishuEnvironmentStatus?.npm_version || "安装 Node.js 后通常会一起提供"}</div>
            </div>
            <div className="rounded border border-gray-100 bg-gray-50 px-3 py-3">
              <div className="text-[11px] text-gray-500">飞书连接组件运行条件</div>
              <div className="mt-1 text-sm font-medium text-gray-900">
                {feishuEnvironmentStatus?.can_start_runtime ? "已准备好" : "暂未满足"}
              </div>
              <div className="mt-1 text-[11px] text-gray-500">{feishuEnvironmentStatus?.error || "完成环境检查后即可继续后续步骤"}</div>
            </div>
          </div>
          {!feishuEnvironmentStatus?.can_start_runtime ? (
            <div className="rounded-lg border border-amber-200 bg-amber-50 px-3 py-2 text-xs text-amber-800">
              当前电脑还没有安装飞书连接所需环境。请先安装 Node.js LTS，完成后重新打开 WorkClaw 或回到这里点击“重新检测”。
            </div>
          ) : null}
        </div>

        {feishuOnboardingEffectiveBranch !== "create_robot" ? (
          <div className="rounded-lg border border-gray-200 bg-white p-4 space-y-3">
            <div>
              <div className="text-sm font-medium text-gray-900">绑定已有机器人</div>
              <div className="text-xs text-gray-500 mt-1">这里只需要填写已有机器人的 App ID 和 App Secret；当前版本不再展示 webhook 相关配置。</div>
            </div>
            <div className="grid grid-cols-1 gap-3 md:grid-cols-2">
              <label className="space-y-1">
                <div className="text-[11px] font-medium text-gray-700">App ID</div>
                <input
                  value={feishuConnectorSettings.app_id}
                  onChange={(event) => onUpdateFeishuConnectorSettings({ app_id: event.target.value })}
                  className="w-full rounded border border-gray-200 bg-gray-50 px-3 py-2 text-sm text-gray-900"
                  placeholder="cli_xxx"
                />
              </label>
              <label className="space-y-1">
                <div className="text-[11px] font-medium text-gray-700">App Secret</div>
                <input
                  type="password"
                  value={feishuConnectorSettings.app_secret}
                  onChange={(event) => onUpdateFeishuConnectorSettings({ app_secret: event.target.value })}
                  className="w-full rounded border border-gray-200 bg-gray-50 px-3 py-2 text-sm text-gray-900"
                  placeholder="填写机器人的 App Secret"
                />
              </label>
            </div>
            {feishuCredentialProbe?.ok ? (
              <div className="rounded-lg border border-emerald-200 bg-emerald-50 px-3 py-2 text-xs text-emerald-800">
                已识别机器人
                {feishuCredentialProbe.bot_name ? `：${feishuCredentialProbe.bot_name}` : ""}。
                {feishuCredentialProbe.bot_open_id ? ` open_id：${feishuCredentialProbe.bot_open_id}` : ""}
              </div>
            ) : null}
            <div className="flex flex-wrap gap-2">
              <button
                type="button"
                onClick={() => void handleValidateFeishuCredentials()}
                disabled={validatingFeishuCredentials}
                className="h-8 px-3 rounded border border-blue-200 bg-white text-xs text-blue-700 hover:bg-blue-50 disabled:bg-gray-100"
              >
                {validatingFeishuCredentials ? "验证中..." : "验证机器人信息"}
              </button>
              <button
                type="button"
                onClick={() => void handleSaveFeishuConnector()}
                disabled={savingFeishuConnector}
                className="h-8 px-3 rounded bg-blue-600 text-xs text-white hover:bg-blue-700 disabled:bg-blue-300"
              >
                {savingFeishuConnector ? "保存中..." : "保存并继续"}
              </button>
            </div>
          </div>
        ) : null}

        <div data-testid="feishu-authorization-step" className="rounded-lg border border-gray-200 bg-white p-4 space-y-3">
          <div>
            <div className="text-sm font-medium text-gray-900">
              {pendingFeishuPairingCount > 0 ? "批准飞书接入请求" : "完成飞书授权"}
            </div>
            <div className="text-xs text-gray-500 mt-1">
              {pendingFeishuPairingCount > 0
                ? "飞书里的机器人已经发来了接入请求。请先在这里批准这次接入，再继续后续配置。"
                : "安装并启动后，请回到飞书中的机器人会话按提示完成授权，然后回到这里刷新状态。"}
            </div>
          </div>
          <div className="rounded-lg border border-gray-100 bg-gray-50 px-3 py-3 text-xs text-gray-700 space-y-1">
            {pendingFeishuPairingCount > 0 ? (
              <>
                <div>1. 飞书里已经生成了 pairing request</div>
                <div>2. 在这里点击“批准这次接入”</div>
                <div>3. 批准后再继续配置接待员工</div>
              </>
            ) : (
              <>
                <div>1. 在飞书中打开机器人会话</div>
                <div>2. 按提示完成授权</div>
                <div>3. 如果机器人提示 access not configured，下一步回来批准接入请求</div>
              </>
            )}
          </div>
          <div className="grid grid-cols-1 gap-3 md:grid-cols-3">
            <div className="rounded border border-gray-100 bg-gray-50 px-3 py-2">
              <div className="text-[11px] text-gray-500">连接组件</div>
              <div className="text-sm font-medium text-gray-900">{feishuSetupProgress?.plugin_installed ? "已安装" : "未安装"}</div>
            </div>
            <div className="rounded border border-gray-100 bg-gray-50 px-3 py-2">
              <div className="text-[11px] text-gray-500">运行状态</div>
              <div className="text-sm font-medium text-gray-900">{officialFeishuRuntimeStatus?.running ? "运行中" : "未启动"}</div>
            </div>
            <div className="rounded border border-gray-100 bg-gray-50 px-3 py-2">
              <div className="text-[11px] text-gray-500">授权状态</div>
              <div className="text-sm font-medium text-gray-900">
                {pendingFeishuPairingCount > 0
                  ? "待批准接入"
                  : feishuSetupProgress?.auth_status === "approved"
                    ? "已完成"
                    : "待完成"}
              </div>
            </div>
          </div>
          {pendingFeishuPairingRequest ? (
            <div className="rounded-lg border border-amber-200 bg-amber-50 px-3 py-3 text-xs text-amber-900 space-y-1">
              <div>发送者：{pendingFeishuPairingRequest.sender_id}</div>
              <div>Pairing Code：{pendingFeishuPairingRequest.code || "未返回"}</div>
              <div>发起时间：{formatCompactDateTime(pendingFeishuPairingRequest.created_at)}</div>
            </div>
          ) : null}
          <div className="flex flex-wrap gap-2">
            <button
              type="button"
              onClick={() => void handleInstallAndStartFeishuConnector()}
              disabled={retryingFeishuConnector || installingOfficialFeishuPlugin}
              className="h-8 px-3 rounded bg-indigo-600 text-xs text-white hover:bg-indigo-700 disabled:bg-indigo-300"
            >
              {retryingFeishuConnector || installingOfficialFeishuPlugin
                ? feishuAuthorizationAction.busyLabel
                : feishuAuthorizationAction.label}
            </button>
            <button
              type="button"
              onClick={() => void handleRefreshFeishuSetup()}
              disabled={retryingFeishuConnector}
              className="h-8 px-3 rounded border border-gray-200 bg-white text-xs text-gray-700 hover:bg-gray-50 disabled:bg-gray-100"
            >
              刷新授权状态
            </button>
            {pendingFeishuPairingRequest ? (
              <>
                <button
                  type="button"
                  onClick={() => void handleResolveFeishuPairingRequest(pendingFeishuPairingRequest.id, "approve")}
                  disabled={feishuPairingActionLoading !== null}
                  className="h-8 px-3 rounded bg-amber-600 text-xs text-white hover:bg-amber-700 disabled:bg-amber-300"
                >
                  {feishuPairingActionLoading === "approve" ? "批准中..." : "批准这次接入"}
                </button>
                <button
                  type="button"
                  onClick={() => void handleResolveFeishuPairingRequest(pendingFeishuPairingRequest.id, "deny")}
                  disabled={feishuPairingActionLoading !== null}
                  className="h-8 px-3 rounded border border-red-200 bg-white text-xs text-red-700 hover:bg-red-50 disabled:bg-gray-100"
                >
                  {feishuPairingActionLoading === "deny" ? "拒绝中..." : "拒绝这次接入"}
                </button>
              </>
            ) : null}
            <button
              type="button"
              onClick={() => void handleStartFeishuInstaller("create")}
              disabled={feishuInstallerBusy}
              className="h-8 px-3 rounded border border-indigo-200 bg-white text-xs text-indigo-700 hover:bg-indigo-50 disabled:bg-gray-100"
            >
              {feishuInstallerBusy && feishuInstallerStartingMode === "create" ? "启动中..." : "新建机器人向导（高级）"}
            </button>
          </div>
          {feishuAuthorizationInlineError && feishuOnboardingHeaderStep !== "authorize" ? (
            <div className="rounded-lg border border-red-200 bg-red-50 px-3 py-2 text-xs text-red-700">
              {feishuAuthorizationInlineError}
            </div>
          ) : null}
          <details
            className="rounded-lg border border-gray-100 bg-gray-50 p-3"
            open={feishuInstallerSession.running || feishuInstallerSession.recent_output.length > 0}
          >
            <summary className="cursor-pointer text-xs font-medium text-gray-700">查看安装向导输出</summary>
            <div className="mt-3 space-y-3">
              <div className="grid grid-cols-1 gap-3 md:grid-cols-3">
                <div className="rounded border border-gray-200 bg-white px-3 py-2">
                  <div className="text-[11px] text-gray-500">向导状态</div>
                  <div className="text-sm font-medium text-gray-900">
                    {feishuInstallerBusy && feishuInstallerStartingMode ? "正在启动" : feishuInstallerSession.running ? "运行中" : "未运行"}
                  </div>
                </div>
                <div className="rounded border border-gray-200 bg-white px-3 py-2">
                  <div className="text-[11px] text-gray-500">当前模式</div>
                  <div className="text-sm font-medium text-gray-900">
                    {feishuInstallerDisplayMode === "create"
                      ? "新建机器人"
                      : feishuInstallerDisplayMode === "link"
                        ? "绑定已有机器人"
                        : "未启动"}
                  </div>
                </div>
                <div className="rounded border border-gray-200 bg-white px-3 py-2">
                  <div className="text-[11px] text-gray-500">提示</div>
                  <div className="text-sm font-medium text-gray-900">
                    {feishuInstallerStartupHint || feishuInstallerSession.prompt_hint || "暂无"}
                  </div>
                </div>
              </div>
              <div className="rounded-lg border border-gray-900 bg-[#050816] px-3 py-3 text-xs text-gray-100">
                <pre className="max-h-72 overflow-auto whitespace-pre-wrap break-all font-mono">
                  {feishuInstallerSession.recent_output.length > 0
                    ? feishuInstallerSession.recent_output.join("\n")
                    : feishuInstallerStartupHint || "暂无安装向导输出"}
                </pre>
              </div>
              <div className="flex flex-col gap-2 md:flex-row">
                <input
                  value={feishuInstallerInput}
                  onChange={(event) => onUpdateFeishuInstallerInput(event.target.value)}
                  placeholder="需要时手动输入，例如 App ID、App Secret 或回车"
                  className="flex-1 rounded border border-gray-200 bg-white px-3 py-2 text-xs text-gray-900"
                />
                <button
                  type="button"
                  onClick={() => void handleSendFeishuInstallerInput()}
                  disabled={feishuInstallerBusy || !feishuInstallerInput.trim()}
                  className="h-9 px-3 rounded border border-gray-200 bg-white text-xs text-gray-700 hover:bg-gray-50 disabled:bg-gray-100"
                >
                  发送输入
                </button>
                <button
                  type="button"
                  onClick={() => void handleStopFeishuInstallerSession()}
                  disabled={feishuInstallerBusy || !feishuInstallerSession.running}
                  className="h-9 px-3 rounded border border-red-200 bg-white text-xs text-red-700 hover:bg-red-50 disabled:bg-gray-100"
                >
                  停止向导
                </button>
              </div>
              <div className="text-[11px] text-gray-500">
                如果你的电脑已安装 OpenClaw，当前向导也会优先命中 WorkClaw 内置的受控 openclaw shim，不会写到外部 OpenClaw 配置。
              </div>
            </div>
          </details>
        </div>

        <div className="rounded-lg border border-gray-200 bg-white p-4 space-y-3">
          <div>
            <div className="text-sm font-medium text-gray-900">接待设置</div>
            <div className="text-xs text-gray-500 mt-1">飞书接通后，还需要指定默认接待员工或配置群聊范围，消息才会稳定落到正确员工。</div>
          </div>
          <div className="rounded-lg border border-blue-100 bg-blue-50 px-3 py-3">
            <div className="text-sm font-medium text-blue-950">{feishuRoutingStatus.label}</div>
            <div className="mt-1 text-xs text-blue-900">{feishuRoutingStatus.description}</div>
          </div>
          <div className="grid grid-cols-1 gap-3 md:grid-cols-3">
            <div className="rounded border border-gray-100 bg-gray-50 px-3 py-2">
              <div className="text-[11px] text-gray-500">授权状态</div>
              <div className="text-sm font-medium text-gray-900">{feishuSetupProgress?.auth_status === "approved" ? "已完成" : "待完成"}</div>
            </div>
            <div className="rounded border border-gray-100 bg-gray-50 px-3 py-2">
              <div className="text-[11px] text-gray-500">默认接待员工</div>
              <div className="text-sm font-medium text-gray-900">{feishuSetupProgress?.default_routing_employee_name || "未设置"}</div>
            </div>
            <div className="rounded border border-gray-100 bg-gray-50 px-3 py-2">
              <div className="text-[11px] text-gray-500">群聊范围规则</div>
              <div className="text-sm font-medium text-gray-900">{feishuSetupProgress?.scoped_routing_count ?? 0} 条</div>
            </div>
          </div>
          <div className="rounded-lg border border-blue-100 bg-blue-50 px-3 py-2 text-xs text-blue-800">
            接待员工的具体配置入口在员工详情页。完成当前接入后，请关闭设置窗口并前往员工详情中的“飞书接待”继续配置。
          </div>
          <div className="flex flex-wrap gap-2">
            <button
              type="button"
              onClick={() => onOpenEmployees?.()}
              className="h-8 px-3 rounded border border-blue-200 bg-white text-xs text-blue-700 hover:bg-blue-50"
            >
              {feishuRoutingStatus.actionLabel}
            </button>
          </div>
        </div>
      </div>
    </details>
  );
}
