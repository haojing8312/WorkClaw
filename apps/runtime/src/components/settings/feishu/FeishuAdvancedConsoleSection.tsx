import { FeishuAuthorizationPanel } from "./FeishuAuthorizationPanel";
import { FeishuEnvironmentPanel } from "./FeishuEnvironmentPanel";
import { FeishuExistingRobotCard } from "./FeishuExistingRobotCard";
import { FeishuRoutingPanel } from "./FeishuRoutingPanel";
import type { FeishuAdvancedConsoleSectionProps } from "./FeishuAdvancedConsoleSection.types";

export type { FeishuAdvancedConsoleSectionProps } from "./FeishuAdvancedConsoleSection.types";

export function FeishuAdvancedConsoleSection(props: FeishuAdvancedConsoleSectionProps) {
  return (
    <details className="rounded-lg border border-gray-200 bg-white p-4">
      <summary className="cursor-pointer text-sm font-medium text-gray-900">飞书接入控制台</summary>
      <div className="mt-4 space-y-3">
        <FeishuEnvironmentPanel
          feishuEnvironmentStatus={props.feishuEnvironmentStatus}
          getFeishuEnvironmentLabel={props.getFeishuEnvironmentLabel}
        />

        {props.feishuOnboardingEffectiveBranch !== "create_robot" ? (
          <FeishuExistingRobotCard
            feishuConnectorSettings={props.feishuConnectorSettings}
            onUpdateFeishuConnectorSettings={props.onUpdateFeishuConnectorSettings}
            feishuCredentialProbe={props.feishuCredentialProbe}
            validatingFeishuCredentials={props.validatingFeishuCredentials}
            savingFeishuConnector={props.savingFeishuConnector}
            handleValidateFeishuCredentials={props.handleValidateFeishuCredentials}
            handleSaveFeishuConnector={props.handleSaveFeishuConnector}
          />
        ) : null}

        <FeishuAuthorizationPanel
          feishuSetupProgress={props.feishuSetupProgress}
          officialFeishuRuntimeStatus={props.officialFeishuRuntimeStatus}
          retryingFeishuConnector={props.retryingFeishuConnector}
          installingOfficialFeishuPlugin={props.installingOfficialFeishuPlugin}
          feishuInstallerSession={props.feishuInstallerSession}
          feishuInstallerInput={props.feishuInstallerInput}
          onUpdateFeishuInstallerInput={props.onUpdateFeishuInstallerInput}
          feishuInstallerBusy={props.feishuInstallerBusy}
          feishuInstallerStartingMode={props.feishuInstallerStartingMode}
          feishuPairingActionLoading={props.feishuPairingActionLoading}
          pendingFeishuPairingCount={props.pendingFeishuPairingCount}
          pendingFeishuPairingRequest={props.pendingFeishuPairingRequest}
          feishuAuthorizationInlineError={props.feishuAuthorizationInlineError}
          feishuOnboardingHeaderStep={props.feishuOnboardingHeaderStep}
          feishuInstallerDisplayMode={props.feishuInstallerDisplayMode}
          feishuInstallerStartupHint={props.feishuInstallerStartupHint}
          feishuAuthorizationAction={props.feishuAuthorizationAction}
          formatCompactDateTime={props.formatCompactDateTime}
          handleInstallAndStartFeishuConnector={props.handleInstallAndStartFeishuConnector}
          handleRefreshFeishuSetup={props.handleRefreshFeishuSetup}
          handleResolveFeishuPairingRequest={props.handleResolveFeishuPairingRequest}
          handleStartFeishuInstaller={props.handleStartFeishuInstaller}
          handleStopFeishuInstallerSession={props.handleStopFeishuInstallerSession}
          handleSendFeishuInstallerInput={props.handleSendFeishuInstallerInput}
        />

        <FeishuRoutingPanel
          onOpenEmployees={props.onOpenEmployees}
          feishuSetupProgress={props.feishuSetupProgress}
          feishuRoutingStatus={props.feishuRoutingStatus}
        />
      </div>
    </details>
  );
}
