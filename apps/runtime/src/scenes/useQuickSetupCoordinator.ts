import { invoke } from "@tauri-apps/api/core";
import { useEffect, useMemo, useRef, useState } from "react";
import {
  buildModelFormFromCatalogItem,
  getModelProviderCatalogItem,
} from "../model-provider-catalog";
import { getModelErrorDisplay } from "../lib/model-error-display";
import {
  applySearchPresetToForm,
  EMPTY_SEARCH_CONFIG_FORM,
  validateSearchConfigForm,
  type SearchConfigFormState,
} from "../lib/search-config";
import type { ModelConfig, ModelConnectionTestResult } from "../types";

const QUICK_FEISHU_SETUP_SKIPPED_KEY = "workclaw:quick-feishu-setup-skipped";

export type QuickModelFormState = ReturnType<typeof buildModelFormFromCatalogItem> & {
  api_key: string;
};

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

export function useQuickSetupCoordinator(options: {
  defaultProviderId: string;
  initialModelSetupCompletedKey: string;
  modelSetupHintDismissedKey: string;
  models: ModelConfig[];
  searchConfigs: ModelConfig[];
  hasHydratedModelConfigs: boolean;
  hasHydratedSearchConfigs: boolean;
  showSettings: boolean;
  loadModels: () => Promise<void>;
  loadSearchConfigs: () => Promise<void>;
  openSettingsAtTab: (tab: "models" | "feishu") => void;
}) {
  const {
    defaultProviderId,
    hasHydratedModelConfigs,
    hasHydratedSearchConfigs,
    initialModelSetupCompletedKey,
    loadModels,
    loadSearchConfigs,
    modelSetupHintDismissedKey,
    models,
    openSettingsAtTab,
    searchConfigs,
    showSettings,
  } = options;

  const defaultProvider = getModelProviderCatalogItem(defaultProviderId);
  const [dismissedModelSetupHint, setDismissedModelSetupHint] = useState(() => {
    if (typeof window === "undefined") {
      return false;
    }
    try {
      return window.localStorage.getItem(modelSetupHintDismissedKey) === "1";
    } catch {
      return false;
    }
  });
  const [hasCompletedInitialModelSetup, setHasCompletedInitialModelSetup] =
    useState(() => {
      if (typeof window === "undefined") {
        return false;
      }
      try {
        return window.localStorage.getItem(initialModelSetupCompletedKey) === "1";
      } catch {
        return false;
      }
    });
  const [showQuickModelSetup, setShowQuickModelSetup] = useState(false);
  const [forceShowModelSetupGate, setForceShowModelSetupGate] = useState(false);
  const [quickSetupStep, setQuickSetupStep] = useState<"model" | "search" | "feishu">(
    "model",
  );
  const [quickModelPresetKey, setQuickModelPresetKey] =
    useState(defaultProvider.id);
  const [quickModelForm, setQuickModelForm] = useState<QuickModelFormState>(
    () => ({
      ...buildModelFormFromCatalogItem(defaultProvider),
      api_key: "",
    }),
  );
  const [quickModelSaving, setQuickModelSaving] = useState(false);
  const [quickModelTesting, setQuickModelTesting] = useState(false);
  const [quickModelTestResult, setQuickModelTestResult] =
    useState<ModelConnectionTestResult | null>(null);
  const [quickModelError, setQuickModelError] = useState("");
  const [quickModelApiKeyVisible, setQuickModelApiKeyVisible] = useState(false);
  const [quickSearchForm, setQuickSearchForm] =
    useState<SearchConfigFormState>(EMPTY_SEARCH_CONFIG_FORM);
  const [quickSearchSaving, setQuickSearchSaving] = useState(false);
  const [quickSearchTesting, setQuickSearchTesting] = useState(false);
  const [quickSearchTestResult, setQuickSearchTestResult] = useState<
    boolean | null
  >(null);
  const [quickSearchError, setQuickSearchError] = useState("");
  const [quickSearchApiKeyVisible, setQuickSearchApiKeyVisible] =
    useState(false);
  const [hasSkippedQuickFeishuSetup, setHasSkippedQuickFeishuSetup] =
    useState(() => {
      if (typeof window === "undefined") {
        return false;
      }
      try {
        return window.localStorage.getItem(QUICK_FEISHU_SETUP_SKIPPED_KEY) === "1";
      } catch {
        return false;
      }
    });
  const quickModelApiKeyInputRef = useRef<HTMLInputElement | null>(null);

  const isBlockingInitialModelSetup =
    !showSettings && !hasCompletedInitialModelSetup;
  const isQuickSetupBusy =
    quickModelSaving ||
    quickModelTesting ||
    quickSearchSaving ||
    quickSearchTesting;
  const canDismissQuickModelSetup =
    !isQuickSetupBusy && !isBlockingInitialModelSetup;
  const selectedQuickModelProvider = getModelProviderCatalogItem(
    quickModelPresetKey,
  );
  const quickModelTestDisplay = quickModelTestResult
    ? getModelErrorDisplay(quickModelTestResult)
    : null;
  const shouldShowQuickModelRawMessage = Boolean(
    quickModelTestDisplay?.rawMessage &&
      quickModelTestDisplay.rawMessage !== quickModelTestDisplay.title &&
      quickModelTestDisplay.rawMessage !== quickModelTestDisplay.message,
  );
  const hasHydratedInitialModelSetupState =
    hasHydratedModelConfigs && hasHydratedSearchConfigs;
  const shouldShowModelSetupGate =
    hasHydratedInitialModelSetupState &&
    (isBlockingInitialModelSetup || forceShowModelSetupGate);
  const shouldShowModelSetupHint =
    hasHydratedInitialModelSetupState &&
    !showSettings &&
    (models.length === 0 || searchConfigs.length === 0) &&
    hasCompletedInitialModelSetup &&
    !dismissedModelSetupHint;

  const resetQuickSetupUiState = () => {
    setQuickSetupStep("model");
    setQuickModelError("");
    setQuickModelTestResult(null);
    setQuickModelApiKeyVisible(false);
    setQuickSearchForm(EMPTY_SEARCH_CONFIG_FORM);
    setQuickSearchError("");
    setQuickSearchTestResult(null);
    setQuickSearchApiKeyVisible(false);
  };

  const getQuickModelConfig = (isDefault: boolean) => ({
    id: "",
    name: quickModelForm.name.trim() || "快速配置模型",
    api_format: quickModelForm.api_format,
    base_url: quickModelForm.base_url.trim(),
    model_name: quickModelForm.model_name.trim(),
    is_default: isDefault,
  });

  const validateQuickModelSetup = () => {
    if (!quickModelForm.base_url.trim()) {
      return "请输入 Base URL";
    }
    if (!quickModelForm.model_name.trim()) {
      return "请输入模型名";
    }
    if (!quickModelForm.api_key.trim()) {
      return "请输入 API Key";
    }
    return null;
  };

  useEffect(() => {
    if (models.length === 0 || searchConfigs.length === 0) {
      return;
    }
    setHasCompletedInitialModelSetup(true);
    setDismissedModelSetupHint(false);
    if (typeof window === "undefined") {
      return;
    }
    try {
      window.localStorage.setItem(initialModelSetupCompletedKey, "1");
      window.localStorage.removeItem(modelSetupHintDismissedKey);
    } catch {
      // ignore
    }
  }, [
    initialModelSetupCompletedKey,
    modelSetupHintDismissedKey,
    models.length,
    searchConfigs.length,
  ]);

  useEffect(() => {
    if (!showQuickModelSetup || typeof window === "undefined") {
      return;
    }
    const focusTimer = window.setTimeout(() => {
      quickModelApiKeyInputRef.current?.focus({ preventScroll: true });
    }, 0);
    return () => {
      window.clearTimeout(focusTimer);
    };
  }, [showQuickModelSetup]);

  useEffect(() => {
    if (!showQuickModelSetup || typeof window === "undefined") {
      return;
    }
    const handleKeyDown = (event: KeyboardEvent) => {
      if (event.key !== "Escape" || !canDismissQuickModelSetup) {
        return;
      }
      event.preventDefault();
      setShowQuickModelSetup(false);
      setForceShowModelSetupGate(false);
      resetQuickSetupUiState();
    };
    window.addEventListener("keydown", handleKeyDown);
    return () => {
      window.removeEventListener("keydown", handleKeyDown);
    };
  }, [canDismissQuickModelSetup, showQuickModelSetup]);

  const actions = useMemo(
    () => ({
      dismissModelSetupHint() {
        setDismissedModelSetupHint(true);
        if (typeof window === "undefined") {
          return;
        }
        try {
          window.localStorage.setItem(modelSetupHintDismissedKey, "1");
        } catch {
          // ignore
        }
      },
      resetFirstUseOnboardingForDevelopment() {
        setHasCompletedInitialModelSetup(false);
        setDismissedModelSetupHint(false);
        setShowQuickModelSetup(false);
        setQuickModelPresetKey(defaultProvider.id);
        setQuickModelForm({
          ...buildModelFormFromCatalogItem(defaultProvider),
          api_key: "",
        });
        resetQuickSetupUiState();
        if (typeof window === "undefined") {
          return;
        }
        try {
          window.localStorage.removeItem(initialModelSetupCompletedKey);
          window.localStorage.removeItem(modelSetupHintDismissedKey);
          window.localStorage.removeItem(QUICK_FEISHU_SETUP_SKIPPED_KEY);
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
        if (!canDismissQuickModelSetup) {
          return;
        }
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
        setQuickSearchForm((current) =>
          applySearchPresetToForm(presetKey, current),
        );
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
        const apiKey = quickModelForm.api_key.trim();
        setQuickModelTesting(true);
        setQuickModelError("");
        setQuickModelTestResult(null);
        try {
          const result = await invoke<ModelConnectionTestResult>(
            "test_connection_cmd",
            {
              config: getQuickModelConfig(false),
              apiKey,
            },
          );
          setQuickModelTestResult(result);
        } catch (error) {
          setQuickModelError(
            extractErrorMessage(error, "模型连接测试失败"),
          );
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
        const apiKey = quickModelForm.api_key.trim();
        setQuickModelSaving(true);
        setQuickModelError("");
        try {
          const savedModelId = await invoke<string>("save_model_config", {
            config: getQuickModelConfig(models.length === 0),
            apiKey,
          });
          if (models.length > 0) {
            await invoke("set_default_model", { modelId: savedModelId });
          }
          await loadModels();
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
              is_default: searchConfigs.length === 0,
            },
            apiKey: quickSearchForm.api_key.trim(),
          });
          setQuickSearchTestResult(ok);
          if (!ok) {
            setQuickSearchError("连接失败，请检查配置");
          }
        } catch (error) {
          setQuickSearchError(
            extractErrorMessage(error, "搜索连接测试失败"),
          );
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
              is_default: searchConfigs.length === 0,
            },
            apiKey: quickSearchForm.api_key.trim(),
          });
          await loadSearchConfigs();
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
          setQuickSearchError(
            extractErrorMessage(error, "保存搜索配置失败"),
          );
        } finally {
          setQuickSearchSaving(false);
        }
      },
      skipQuickSearchSetup() {
        if (isBlockingInitialModelSetup || quickSearchSaving || quickSearchTesting) {
          return;
        }
        setShowQuickModelSetup(false);
        setQuickSetupStep("model");
        setQuickSearchForm(EMPTY_SEARCH_CONFIG_FORM);
        setQuickSearchError("");
        setQuickSearchTestResult(null);
        setQuickSearchApiKeyVisible(false);
      },
      skipQuickFeishuSetup() {
        if (isQuickSetupBusy) {
          return;
        }
        setHasSkippedQuickFeishuSetup(true);
        if (typeof window !== "undefined") {
          try {
            window.localStorage.setItem(QUICK_FEISHU_SETUP_SKIPPED_KEY, "1");
          } catch {
            // ignore
          }
        }
        setShowQuickModelSetup(false);
        setForceShowModelSetupGate(false);
        setQuickSetupStep("model");
      },
      openQuickFeishuSetupFromDialog() {
        if (isQuickSetupBusy) {
          return;
        }
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
      isBlockingInitialModelSetup,
      isQuickSetupBusy,
      loadModels,
      loadSearchConfigs,
      modelSetupHintDismissedKey,
      models.length,
      openSettingsAtTab,
      quickModelForm,
      quickModelSaving,
      quickModelTesting,
      quickSearchForm,
      quickSearchSaving,
      quickSearchTesting,
      searchConfigs.length,
    ],
  );

  return {
    actions,
    canDismissQuickModelSetup,
    hasCompletedInitialModelSetup,
    hasHydratedInitialModelSetupState,
    isBlockingInitialModelSetup,
    isQuickSetupBusy,
    quickModelApiKeyInputRef,
    quickModelApiKeyVisible,
    quickModelError,
    quickModelForm,
    quickModelPresetKey,
    quickModelSaving,
    quickModelTestDisplay,
    quickModelTestResult,
    quickModelTesting,
    quickSearchApiKeyVisible,
    quickSearchError,
    quickSearchForm,
    quickSearchSaving,
    quickSearchTestResult,
    quickSearchTesting,
    quickSetupStep,
    selectedQuickModelProvider,
    setQuickModelApiKeyVisible,
    setQuickModelError,
    setQuickModelForm,
    setQuickModelTestResult,
    setQuickSearchApiKeyVisible,
    setQuickSearchError,
    setQuickSearchForm,
    setQuickSearchTestResult,
    shouldShowModelSetupGate,
    shouldShowModelSetupHint,
    shouldShowQuickModelRawMessage,
    showQuickModelSetup,
  };
}
