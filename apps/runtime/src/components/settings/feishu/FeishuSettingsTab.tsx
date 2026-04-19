import { useCallback, useRef, type ComponentProps } from "react";
import { FeishuAdvancedConsoleSection } from "./FeishuAdvancedConsoleSection";
import { FeishuAdvancedSection } from "./FeishuAdvancedSection";
import { FeishuSettingsSection } from "./FeishuSettingsSection";
import { ChannelRegistrySection } from "../channels/ChannelRegistrySection";

interface FeishuSettingsTabProps {
  onOpenEmployees?: () => void;
  channelRegistrySectionProps: ComponentProps<typeof ChannelRegistrySection>;
  settingsSectionProps: ComponentProps<typeof FeishuSettingsSection>;
  advancedConsoleSectionProps: ComponentProps<typeof FeishuAdvancedConsoleSection>;
  advancedSectionProps: ComponentProps<typeof FeishuAdvancedSection>;
}

export function FeishuSettingsTab({
  onOpenEmployees,
  channelRegistrySectionProps,
  settingsSectionProps,
  advancedConsoleSectionProps,
  advancedSectionProps,
}: FeishuSettingsTabProps) {
  const advancedSectionRef = useRef<HTMLDivElement | null>(null);

  const handleOpenFeishuAdvancedSettings = useCallback(() => {
    const container = advancedSectionRef.current;
    if (!container) {
      return;
    }
    const details = container.querySelector("details");
    if (details instanceof HTMLDetailsElement) {
      details.open = true;
      if (typeof details.scrollIntoView === "function") {
        details.scrollIntoView({ behavior: "auto", block: "start" });
      }
      const summary = details.querySelector("summary");
      if (summary instanceof HTMLElement) {
        summary.focus();
      }
      return;
    }
    if (typeof container.scrollIntoView === "function") {
      container.scrollIntoView({ behavior: "auto", block: "start" });
    }
  }, []);

  return (
    <div className="space-y-3">
      <ChannelRegistrySection
        {...channelRegistrySectionProps}
        onOpenEmployees={onOpenEmployees}
        onOpenFeishuAdvancedSettings={handleOpenFeishuAdvancedSettings}
      />
      <FeishuSettingsSection onOpenEmployees={onOpenEmployees} {...settingsSectionProps} />
      <FeishuAdvancedConsoleSection
        onOpenEmployees={onOpenEmployees}
        {...advancedConsoleSectionProps}
      />
      <div ref={advancedSectionRef}>
        <FeishuAdvancedSection {...advancedSectionProps} />
      </div>
    </div>
  );
}
