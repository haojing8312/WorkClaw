import { useState } from "react";
import { useSettingsController } from "../scenes/settings/useSettingsController";
import { CapabilityRoutingSection } from "./settings/CapabilityRoutingSection";
import { DesktopSettingsSection } from "./settings/desktop/DesktopSettingsSection";
import { FeishuSettingsTab } from "./settings/feishu/FeishuSettingsTab";
import { useFeishuSettingsController } from "./settings/feishu/useFeishuSettingsController";
import { useChannelRegistryController } from "./settings/channels/useChannelRegistryController";
import { McpSettingsSection } from "./settings/mcp/McpSettingsSection";
import { ModelsSettingsSection } from "./settings/models/ModelsSettingsSection";
import { RoutingSettingsSection } from "./settings/routing/RoutingSettingsSection";
import { SearchSettingsSection } from "./settings/search/SearchSettingsSection";
import { HealthSettingsSection } from "./settings/HealthSettingsSection";
import { SettingsShell } from "./settings/SettingsShell";
import { SettingsTabNav, type SettingsTabName } from "./settings/SettingsTabNav";

export { buildFeishuOnboardingState } from "./settings/feishu/feishuSelectors";
export type {
  FeishuOnboardingInput,
  FeishuOnboardingState,
  FeishuOnboardingStep,
} from "./settings/feishu/feishuSelectors";

interface Props {
  onClose: () => void;
  onOpenEmployees?: () => void;
  initialTab?: SettingsTabName;
  showDevModelSetupTools?: boolean;
  onDevResetFirstUseOnboarding?: () => void;
  onDevOpenQuickModelSetup?: () => void;
}

const ROUTING_CAPABILITIES = [
  { label: "对话 Chat", value: "chat" },
  { label: "视觉 Vision", value: "vision" },
  { label: "生图 Image", value: "image_gen" },
  { label: "语音转写 STT", value: "audio_stt" },
  { label: "语音合成 TTS", value: "audio_tts" },
];

const SHOW_CAPABILITY_ROUTING_SETTINGS = false;
const SHOW_HEALTH_SETTINGS = false;
const SHOW_MCP_SETTINGS = true;
const SHOW_AUTO_ROUTING_SETTINGS = false;

export function SettingsView({
  onClose,
  onOpenEmployees,
  initialTab = "models",
  showDevModelSetupTools = false,
  onDevResetFirstUseOnboarding,
  onDevOpenQuickModelSetup,
}: Props) {
  const [activeTab, setActiveTab] = useState<SettingsTabName>(initialTab);
  const {
    models,
    setModels,
    providers,
    setProviders,
    selectedCapability,
    setSelectedCapability,
    chatRoutingPolicy,
    setChatRoutingPolicy,
    policySaveState,
    policyError,
    chatPrimaryModels,
    chatFallbackRows,
    routeTemplates,
    selectedRouteTemplateId,
    setSelectedRouteTemplateId,
    healthResult,
    allHealthResults,
    healthLoading,
    healthProviderId,
    setHealthProviderId,
    routeLogsLoading,
    setRouteLogsOffset,
    routeLogsHasMore,
    routeLogsSessionId,
    setRouteLogsSessionId,
    routeLogsCapabilityFilter,
    setRouteLogsCapabilityFilter,
    routeLogsResultFilter,
    setRouteLogsResultFilter,
    routeLogsErrorKindFilter,
    setRouteLogsErrorKindFilter,
    routeLogsExporting,
    routeStats,
    routeStatsLoading,
    routeStatsCapability,
    setRouteStatsCapability,
    routeStatsHours,
    setRouteStatsHours,
    filteredRouteLogs,
    getCapabilityRecommendedDefaults,
    loadChatPrimaryModels,
    loadCapabilityRoutingPolicy,
    loadRouteTemplates,
    handleSaveChatPolicy,
    handleCheckProviderHealth,
    handleCheckAllProviderHealth,
    loadRecentRouteLogs,
    loadRouteStats,
    handleExportRouteLogsCsv,
    addFallbackRow,
    updateFallbackRow,
    removeFallbackRow,
    handleApplyRouteTemplate,
  } = useSettingsController();
  const {
    sections: {
      settingsSectionProps,
      advancedConsoleSectionProps,
      advancedSectionProps,
    },
  } = useFeishuSettingsController({ activeTab });
  const channelRegistryController = useChannelRegistryController({ activeTab });

  const feishuSettingsSectionProps = {
    ...settingsSectionProps,
    feishuRoutingActionAvailable: Boolean(onOpenEmployees),
    feishuOnboardingPrimaryActionLabel:
      settingsSectionProps.feishuOnboardingHeaderStep === "routing" && !onOpenEmployees
        ? "请从员工中心继续"
        : settingsSectionProps.feishuOnboardingPrimaryActionLabel,
    feishuOnboardingPrimaryActionDisabled:
      settingsSectionProps.feishuOnboardingHeaderStep === "routing" && !onOpenEmployees
        ? true
        : settingsSectionProps.feishuOnboardingPrimaryActionDisabled,
  };

  const inputCls = "sm-input w-full text-sm py-1.5";
  const labelCls = "sm-field-label";

  function handleCapabilityChange(capability: string) {
    setSelectedCapability(capability);
    loadCapabilityRoutingPolicy(capability);
    loadRouteTemplates(capability);
  }

  function handlePrimaryProviderChange(providerId: string) {
    setChatRoutingPolicy((state) => ({ ...state, primary_provider_id: providerId }));
    void loadChatPrimaryModels(providerId, selectedCapability);
  }

  function handleApplyRecommendedDefaults() {
    const defaults = getCapabilityRecommendedDefaults(selectedCapability);
    setChatRoutingPolicy((state) => ({
      ...state,
      timeout_ms: defaults.timeout_ms,
      retry_count: defaults.retry_count,
    }));
  }

  function copyTextToClipboard(text: string) {
    return navigator?.clipboard?.writeText?.(text);
  }

  return (
    <SettingsShell
      onClose={onClose}
      tabs={
        <SettingsTabNav
          activeTab={activeTab}
          onSelectTab={setActiveTab}
          showCapabilityRoutingSettings={SHOW_CAPABILITY_ROUTING_SETTINGS}
          showHealthSettings={SHOW_HEALTH_SETTINGS}
          showMcpSettings={SHOW_MCP_SETTINGS}
          showAutoRoutingSettings={SHOW_AUTO_ROUTING_SETTINGS}
        />
      }
    >
      {activeTab === "models" && (
        <div className="space-y-4">
          <ModelsSettingsSection
            models={models}
            providers={providers}
            onModelsChange={setModels}
            onProvidersChange={setProviders}
            showDevModelSetupTools={showDevModelSetupTools}
            onDevResetFirstUseOnboarding={onDevResetFirstUseOnboarding}
            onDevOpenQuickModelSetup={onDevOpenQuickModelSetup}
          />
        </div>
      )}

      <DesktopSettingsSection models={models} visible={activeTab === "desktop"} />

      {SHOW_CAPABILITY_ROUTING_SETTINGS && activeTab === "capabilities" && (
        <CapabilityRoutingSection
          capabilities={ROUTING_CAPABILITIES}
          chatFallbackRows={chatFallbackRows}
          chatPrimaryModels={chatPrimaryModels}
          chatRoutingPolicy={chatRoutingPolicy}
          inputCls={inputCls}
          labelCls={labelCls}
          policyError={policyError}
          policySaveState={policySaveState}
          providers={providers}
          routeTemplates={routeTemplates}
          selectedCapability={selectedCapability}
          selectedRouteTemplateId={selectedRouteTemplateId}
          onAddFallbackRow={addFallbackRow}
          onApplyRecommendedDefaults={handleApplyRecommendedDefaults}
          onApplyRouteTemplate={handleApplyRouteTemplate}
          onCapabilityChange={handleCapabilityChange}
          onPrimaryModelChange={(primaryModel) =>
            setChatRoutingPolicy((state) => ({ ...state, primary_model: primaryModel }))
          }
          onPrimaryProviderChange={handlePrimaryProviderChange}
          onRemoveFallbackRow={removeFallbackRow}
          onSaveChatPolicy={handleSaveChatPolicy}
          onSelectedRouteTemplateIdChange={setSelectedRouteTemplateId}
          onTimeoutChange={(timeoutMs) =>
            setChatRoutingPolicy((state) => ({ ...state, timeout_ms: timeoutMs }))
          }
          onRetryCountChange={(retryCount) =>
            setChatRoutingPolicy((state) => ({ ...state, retry_count: retryCount }))
          }
          onToggleEnabled={(enabled) => setChatRoutingPolicy((state) => ({ ...state, enabled }))}
          onUpdateFallbackRow={updateFallbackRow}
        />
      )}

      {SHOW_HEALTH_SETTINGS && activeTab === "health" && (
        <HealthSettingsSection
          allHealthResults={allHealthResults}
          filteredRouteLogs={filteredRouteLogs}
          healthLoading={healthLoading}
          healthProviderId={healthProviderId}
          healthResult={healthResult}
          inputCls={inputCls}
          providers={providers}
          routeLogsCapabilityFilter={routeLogsCapabilityFilter}
          routeLogsErrorKindFilter={routeLogsErrorKindFilter}
          routeLogsExporting={routeLogsExporting}
          routeLogsHasMore={routeLogsHasMore}
          routeLogsLoading={routeLogsLoading}
          routeLogsResultFilter={routeLogsResultFilter}
          routeLogsSessionId={routeLogsSessionId}
          routeStats={routeStats}
          routeStatsCapability={routeStatsCapability}
          routeStatsHours={routeStatsHours}
          routeStatsLoading={routeStatsLoading}
          onCheckAllProviderHealth={handleCheckAllProviderHealth}
          onCheckProviderHealth={handleCheckProviderHealth}
          onCopyRouteLogError={copyTextToClipboard}
          onCopyRouteLogSessionId={copyTextToClipboard}
          onExportRouteLogsCsv={handleExportRouteLogsCsv}
          onLoadMoreRouteLogs={() => loadRecentRouteLogs(true)}
          onLoadRouteStats={loadRouteStats}
          onRefreshRouteLogs={() => {
            setRouteLogsOffset(0);
            return loadRecentRouteLogs(false);
          }}
          onRouteLogsCapabilityFilterChange={setRouteLogsCapabilityFilter}
          onRouteLogsErrorKindFilterChange={setRouteLogsErrorKindFilter}
          onRouteLogsResultFilterChange={setRouteLogsResultFilter}
          onRouteLogsSessionIdChange={setRouteLogsSessionId}
          onRouteStatsCapabilityChange={setRouteStatsCapability}
          onRouteStatsHoursChange={setRouteStatsHours}
          onSelectHealthProvider={setHealthProviderId}
        />
      )}

      {SHOW_MCP_SETTINGS && activeTab === "mcp" && <McpSettingsSection />}

      {activeTab === "search" && <SearchSettingsSection />}

      {activeTab === "feishu" && (
        <FeishuSettingsTab
          onOpenEmployees={onOpenEmployees}
          channelRegistrySectionProps={{
            entries: channelRegistryController.entries,
            loading: channelRegistryController.loading,
            error: channelRegistryController.error,
            feishuHostPanel: channelRegistryController.feishuHostPanel,
            wecomHostPanel: channelRegistryController.wecomHostPanel,
            wecomPanel: channelRegistryController.wecomPanel,
            onRefresh: channelRegistryController.refresh,
          }}
          settingsSectionProps={feishuSettingsSectionProps}
          advancedConsoleSectionProps={advancedConsoleSectionProps}
          advancedSectionProps={advancedSectionProps}
        />
      )}

      {SHOW_AUTO_ROUTING_SETTINGS && activeTab === "routing" && <RoutingSettingsSection />}
    </SettingsShell>
  );
}
