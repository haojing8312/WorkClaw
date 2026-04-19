import type { OpenClawPluginFeishuAdvancedSettings } from "../../../types";

export type FeishuAdvancedFieldConfig = {
  key: keyof OpenClawPluginFeishuAdvancedSettings;
  label: string;
  description: string;
  kind: "input" | "textarea";
  rows?: number;
};

export interface FeishuAdvancedSectionProps {
  feishuAdvancedSettings: OpenClawPluginFeishuAdvancedSettings;
  onUpdateFeishuAdvancedSettings: (patch: Partial<OpenClawPluginFeishuAdvancedSettings>) => void;
  savingFeishuAdvancedSettings: boolean;
  onSaveFeishuAdvancedSettings: () => Promise<void>;
}
