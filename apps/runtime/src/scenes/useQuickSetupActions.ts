import { invoke } from "@tauri-apps/api/core";
import { useMemo } from "react";
import type { Dispatch, SetStateAction } from "react";
import {
  buildModelFormFromCatalogItem,
  getModelProviderCatalogItem,
} from "../model-provider-catalog";
import {
  applySearchPresetToForm,
  EMPTY_SEARCH_CONFIG_FORM,
  validateSearchConfigForm,
  type SearchConfigFormState,
} from "../lib/search-config";
import { storageKey } from "../lib/branding";
import type { ModelConnectionTestResult } from "../types";
import type { QuickModelFormState } from "./useQuickSetupCoordinator";

function extractErrorMessage(error: unknown, fallback: string): string {
  if (typeof error === "string") {
    return error;
  }
  if (error instanceof Error) {
    return error.message || fallback;
  }
  if (
    typeof error === "object" &&
    error !== null &&
    "message" in error &&
    typeof (error as { message?: unknown }).message === "string"
  ) {
    return (error as { message: string }).message;
  }
  return fallback;
}

interface UseQuickSetupActionsOptions {
  canDismissQuickModelSetup: boolean;
  defaultProvider: ReturnType<typeof getModelProviderCatalogItem>;
  hasSkippedQuickFeishuSetup: boolean;
  hasSkippedQuickSearchSetup: boolean;
  initialModelSetupCompletedKey: string;
  isBlockingInitialModelSetup: boolean;
  isQuickSetupBusy: boolean;
  loadModels: () => Promise<void>;
  loadSearchConfigs: () => Promise<void>;
  modelSetupHintDismissedKey: string;
  modelCount: number;
  openSettingsAtTab: (tab: "models" | "feishu") => void;
  quickModelForm: QuickModelFormState;
  quickModelSaving: boolean;
  quickModelTesting: boolean;
  quickSearchForm: SearchConfigFormState;
  quickSearchSaving: boolean;
  quickSearchTesting: boolean;
  resetQuickSetupUiState: () => void;
  searchConfigCount: number;
  setDismissedModelSetupHint: (value: boolean) => void;
  setForceShowModelSetupGate: (value: boolean) => void;
  setHasCompletedInitialModelSetup: (value: boolean) => void;
  setHasSkippedQuickFeishuSetup: (value: boolean) => void;
  setHasSkippedQuickSearchSetup: (value: boolean) => void;
  setQuickModelApiKeyVisible: (value: boolean) => void;
  setQuickModelError: (value: string) => void;
  setQuickModelForm: Dispatch<SetStateAction<QuickModelFormState>>;
  setQuickModelPresetKey: (value: string) => void;
  setQuickModelSaving: (value: boolean) => void;
  setQuickModelTestResult: (value: ModelConnectionTestResult | null) => void;
  setQuickModelTesting: (value: boolean) => void;
  setQuickSearchApiKeyVisible: (value: boolean) => void;
  setQuickSearchError: (value: string) => void;
  setQuickSearchForm: Dispatch<SetStateAction<SearchConfigFormState>>;
  setQuickSearchSaving: (value: boolean) => void;
  setQuickSearchTestResult: (value: boolean | null) => void;
  setQuickSearchTesting: (value: boolean) => void;
  setQuickSetupStep: (value: "model" | "search" | "feishu") => void;
  setShowQuickModelSetup: (value: boolean) => void;
  validateQuickModelSetup: () => string | null;
}

export function useQuickSetupActions(options: UseQuickSetupActionsOptions) {
  const {
    canDismissQuickModelSetup,
    defaultProvider,
    hasSkippedQuickFeishuSetup,
    hasSkippedQuickSearchSetup,
    initialModelSetupCompletedKey,
    isBlockingInitialModelSetup,
    isQuickSetupBusy,
    loadModels,
    loadSearchConfigs,
    modelCount,
    modelSetupHintDismissedKey,
    openSettingsAtTab,
    quickModelForm,
    quickModelSaving,
    quickModelTesting,
    quickSearchForm,
    quickSearchSaving,
    quickSearchTesting,
    resetQuickSetupUiState,
    searchConfigCount,
    setDismissedModelSetupHint,
    setForceShowModelSetupGate,
    setHasCompletedInitialModelSetup,
    setHasSkippedQuickFeishuSetup,
    setHasSkippedQuickSearchSetup,
    setQuickModelApiKeyVisible,
    setQuickModelError,
    setQuickModelForm,
    setQuickModelPresetKey,
    setQuickModelSaving,
    setQuickModelTestResult,
    setQuickModelTesting,
    setQuickSearchApiKeyVisible,
    setQuickSearchError,
    setQuickSearchForm,
    setQuickSearchSaving,
    setQuickSearchTestResult,
    setQuickSearchTesting,
    setQuickSetupStep,
    setShowQuickModelSetup,
    validateQuickModelSetup,
  } = options;

  const getQuickModelConfig = (isDefault: boolean) => ({
    id: "",
    name: quickModelForm.name.trim() || "快速配置模型",
    api_format: quickModelForm.api_format,
    base_url: quickModelForm.base_url.trim(),
    model_name: quickModelForm.model_name.trim(),
    is_default: isDefault,
  });

  return useMemo(
    () => ({
      dismissModelSetupHint() {
        setDismissedModelSetupHint(true);
        if (typeof window === "undefined") return;
        try {
          window.localStorage.setItem(modelSetupHintDismissedKey, "1");
        } catch {
          // ignore
        }
      },
      resetFirstUseOnboardingForDevelopment() {
        setHasCompletedInitialModelSetup(false);
        setDismissedModelSetupHint(false);
        setHasSkippedQuickSearchSetup(false);
        setShowQuickModelSetup(false);
        setQuickModelPresetKey(defaultProvider.id);
        setQuickModelForm({
          ...buildModelFormFromCatalogItem(defaultProvider),
          api_key: "",
        });
        resetQuickSetupUiState();
        if (typeof window === "undefined") return;
        try {
          window.localStorage.removeItem(initialModelSetupCompletedKey);
          window.localStorage.removeItem(modelSetupHintDismissedKey);
          window.localStorage.removeItem(optionsKeys.quickFeishuSkipped);
          window.localStorage.removeItem(optionsKeys.quickSearchSkipped);
        } catch {
          // ignore
        }
        setHasSkippedQuickFeishuSetup(false);
      },
      openQuickModelSetup() {
        setShowQuickModelSetup(true);
        resetQuickSetupUiState();
      },
      openInitialModelSetupGate() {
        setForceShowModelSetupGate(true);
        setShowQuickModelSetup(true);
        resetQuickSetupUiState();
      },
      closeQuickModelSetup() {
        if (!canDismissQuickModelSetup) return;
        setShowQuickModelSetup(false);
        setForceShowModelSetupGate(false);
        resetQuickSetupUiState();
      },
      openSettingsForModelSetup() {
        setShowQuickModelSetup(false);
        setForceShowModelSetupGate(false);
        resetQuickSetupUiState();
        openSettingsAtTab("models");
      },
      applyQuickModelPreset(presetKey: string) {
        const provider = getModelProviderCatalogItem(presetKey);
        setQuickModelPresetKey(provider.id);
        setQuickModelForm((prev) => ({
          ...prev,
          ...buildModelFormFromCatalogItem(provider),
          api_key: prev.api_key,
        }));
        setQuickModelTestResult(null);
        setQuickModelError("");
      },
      applyQuickSearchPreset(presetKey: string) {
        setQuickSearchForm((current) => applySearchPresetToForm(presetKey, current));
        setQuickSearchError("");
        setQuickSearchTestResult(null);
      },
      async testQuickModelSetupConnection() {
        if (quickModelSaving || quickModelTesting) return;
        const validationError = validateQuickModelSetup();
        if (validationError) {
          setQuickModelError(validationError);
          setQuickModelTestResult(null);
          return;
        }
        setQuickModelTesting(true);
        setQuickModelError("");
        setQuickModelTestResult(null);
        try {
          const result = await invoke<ModelConnectionTestResult>("test_connection_cmd", {
            config: getQuickModelConfig(false),
            apiKey: quickModelForm.api_key.trim(),
          });
          setQuickModelTestResult(result);
        } catch (error) {
          setQuickModelError(extractErrorMessage(error, "模型连接测试失败"));
          setQuickModelTestResult(null);
        } finally {
          setQuickModelTesting(false);
        }
      },
      async saveQuickModelSetup() {
        if (quickModelSaving || quickModelTesting) return;
        const validationError = validateQuickModelSetup();
        if (validationError) {
          setQuickModelError(validationError);
          setQuickModelTestResult(null);
          return;
        }
        setQuickModelSaving(true);
        setQuickModelError("");
        try {
          const savedModelId = await invoke<string>("save_model_config", {
            config: getQuickModelConfig(modelCount === 0),
            apiKey: quickModelForm.api_key.trim(),
          });
          if (modelCount > 0) {
            await invoke("set_default_model", { modelId: savedModelId });
          }
          await loadModels();
          setHasSkippedQuickSearchSetup(false);
          if (typeof window !== "undefined") {
            try {
              window.localStorage.removeItem(optionsKeys.quickSearchSkipped);
            } catch {
              // ignore
            }
          }
          setQuickModelForm((prev) => ({ ...prev, api_key: "" }));
          setQuickModelTestResult(null);
          setQuickModelApiKeyVisible(false);
          setQuickSetupStep("search");
        } catch (error) {
          setQuickModelError(String(error));
        } finally {
          setQuickModelSaving(false);
        }
      },
      async testQuickSearchSetupConnection() {
        if (quickSearchSaving || quickSearchTesting) return;
        const validationError = validateSearchConfigForm(quickSearchForm);
        if (validationError) {
          setQuickSearchError(validationError);
          setQuickSearchTestResult(null);
          return;
        }
        setQuickSearchTesting(true);
        setQuickSearchError("");
        setQuickSearchTestResult(null);
        try {
          const ok = await invoke<boolean>("test_search_connection", {
            config: {
              id: "",
              name: quickSearchForm.name.trim(),
              api_format: quickSearchForm.api_format,
              base_url: quickSearchForm.base_url.trim(),
              model_name: quickSearchForm.model_name.trim(),
              is_default: searchConfigCount === 0,
            },
            apiKey: quickSearchForm.api_key.trim(),
          });
          setQuickSearchTestResult(ok);
          if (!ok) {
            setQuickSearchError("连接失败，请检查配置");
          }
        } catch (error) {
          setQuickSearchError(extractErrorMessage(error, "搜索连接测试失败"));
          setQuickSearchTestResult(false);
        } finally {
          setQuickSearchTesting(false);
        }
      },
      async saveQuickSearchSetup() {
        if (quickSearchSaving || quickSearchTesting) return;
        const validationError = validateSearchConfigForm(quickSearchForm);
        if (validationError) {
          setQuickSearchError(validationError);
          setQuickSearchTestResult(null);
          return;
        }
        setQuickSearchSaving(true);
        setQuickSearchError("");
        try {
          await invoke("save_model_config", {
            config: {
              id: "",
              name: quickSearchForm.name.trim(),
              api_format: quickSearchForm.api_format,
              base_url: quickSearchForm.base_url.trim(),
              model_name: quickSearchForm.model_name.trim(),
              is_default: searchConfigCount === 0,
            },
            apiKey: quickSearchForm.api_key.trim(),
          });
          await loadSearchConfigs();
          setHasSkippedQuickSearchSetup(false);
          if (typeof window !== "undefined") {
            try {
              window.localStorage.removeItem(optionsKeys.quickSearchSkipped);
            } catch {
              // ignore
            }
          }
          if (isBlockingInitialModelSetup && !hasSkippedQuickFeishuSetup) {
            setQuickSetupStep("feishu");
            return;
          }
          setShowQuickModelSetup(false);
          setForceShowModelSetupGate(false);
          setQuickSetupStep("model");
          setQuickSearchForm(EMPTY_SEARCH_CONFIG_FORM);
          setQuickSearchTestResult(null);
          setQuickSearchApiKeyVisible(false);
        } catch (error) {
          setQuickSearchError(extractErrorMessage(error, "保存搜索配置失败"));
        } finally {
          setQuickSearchSaving(false);
        }
      },
      skipQuickSearchSetup() {
        if (quickSearchSaving || quickSearchTesting) return;
        setHasSkippedQuickSearchSetup(true);
        setHasCompletedInitialModelSetup(true);
        setDismissedModelSetupHint(true);
        if (typeof window !== "undefined") {
          try {
            window.localStorage.setItem(optionsKeys.quickSearchSkipped, "1");
            window.localStorage.setItem(initialModelSetupCompletedKey, "1");
            window.localStorage.setItem(modelSetupHintDismissedKey, "1");
          } catch {
            // ignore
          }
        }
        if (isBlockingInitialModelSetup && !hasSkippedQuickFeishuSetup) {
          setQuickSetupStep("feishu");
          setQuickSearchForm(EMPTY_SEARCH_CONFIG_FORM);
          setQuickSearchError("");
          setQuickSearchTestResult(null);
          setQuickSearchApiKeyVisible(false);
          return;
        }
        setShowQuickModelSetup(false);
        setForceShowModelSetupGate(false);
        setQuickSetupStep("model");
        setQuickSearchForm(EMPTY_SEARCH_CONFIG_FORM);
        setQuickSearchError("");
        setQuickSearchTestResult(null);
        setQuickSearchApiKeyVisible(false);
      },
      skipQuickFeishuSetup() {
        if (isQuickSetupBusy) return;
        setHasSkippedQuickFeishuSetup(true);
        if (typeof window !== "undefined") {
          try {
            window.localStorage.setItem(optionsKeys.quickFeishuSkipped, "1");
          } catch {
            // ignore
          }
        }
        setShowQuickModelSetup(false);
        setForceShowModelSetupGate(false);
        setQuickSetupStep("model");
      },
      openQuickFeishuSetupFromDialog() {
        if (isQuickSetupBusy) return;
        setShowQuickModelSetup(false);
        setForceShowModelSetupGate(false);
        setQuickSetupStep("model");
        openSettingsAtTab("feishu");
      },
    }),
    [
      canDismissQuickModelSetup,
      defaultProvider,
      hasSkippedQuickFeishuSetup,
      initialModelSetupCompletedKey,
      hasSkippedQuickSearchSetup,
      isBlockingInitialModelSetup,
      isQuickSetupBusy,
      loadModels,
      loadSearchConfigs,
      modelCount,
      modelSetupHintDismissedKey,
      openSettingsAtTab,
      quickModelForm,
      quickModelSaving,
      quickModelTesting,
      quickSearchForm,
      quickSearchSaving,
      quickSearchTesting,
      resetQuickSetupUiState,
      searchConfigCount,
      setDismissedModelSetupHint,
      setForceShowModelSetupGate,
      setHasCompletedInitialModelSetup,
      setHasSkippedQuickFeishuSetup,
      setHasSkippedQuickSearchSetup,
      setQuickModelApiKeyVisible,
      setQuickModelError,
      setQuickModelForm,
      setQuickModelPresetKey,
      setQuickModelSaving,
      setQuickModelTestResult,
      setQuickModelTesting,
      setQuickSearchApiKeyVisible,
      setQuickSearchError,
      setQuickSearchForm,
      setQuickSearchSaving,
      setQuickSearchTestResult,
      setQuickSearchTesting,
      setQuickSetupStep,
      setShowQuickModelSetup,
      validateQuickModelSetup,
    ],
  );
}

const optionsKeys = {
  quickFeishuSkipped: storageKey("quick-feishu-setup-skipped"),
  quickSearchSkipped: storageKey("quick-search-setup-skipped"),
};
