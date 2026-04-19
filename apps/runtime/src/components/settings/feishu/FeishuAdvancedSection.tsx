import { FeishuAdvancedSettingsForm } from "./FeishuAdvancedSettingsForm";
import type { FeishuAdvancedSectionProps } from "./FeishuAdvancedSection.types";

export type { FeishuAdvancedSectionProps } from "./FeishuAdvancedSection.types";

export function FeishuAdvancedSection(props: FeishuAdvancedSectionProps) {
  return (
    <>
      <FeishuAdvancedSettingsForm
        feishuAdvancedSettings={props.feishuAdvancedSettings}
        onUpdateFeishuAdvancedSettings={props.onUpdateFeishuAdvancedSettings}
        savingFeishuAdvancedSettings={props.savingFeishuAdvancedSettings}
        onSaveFeishuAdvancedSettings={props.onSaveFeishuAdvancedSettings}
      />
    </>
  );
}
