import { useEffect, useRef, useState } from "react";
import {
  buildModelFormFromCatalogItem,
  getModelProviderCatalogItem,
} from "../model-provider-catalog";
import { getModelErrorDisplay } from "../lib/model-error-display";
import {
  EMPTY_SEARCH_CONFIG_FORM,
  type SearchConfigFormState,
} from "../lib/search-config";
import { storageKey } from "../lib/branding";
import type { ModelConfig, ModelConnectionTestResult } from "../types";
import { useQuickSetupActions } from "./useQuickSetupActions";

const QUICK_FEISHU_SETUP_SKIPPED_KEY = storageKey("quick-feishu-setup-skipped");
const QUICK_SEARCH_SETUP_SKIPPED_KEY = storageKey("quick-search-setup-skipped");

export type QuickModelFormState = ReturnType<typeof buildModelFormFromCatalogItem> & {
  api_key: string;
};

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
  const [hasSkippedQuickSearchSetup, setHasSkippedQuickSearchSetup] =
    useState(() => {
      if (typeof window === "undefined") {
        return false;
      }
      try {
        return window.localStorage.getItem(QUICK_SEARCH_SETUP_SKIPPED_KEY) === "1";
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
    !hasSkippedQuickSearchSetup &&
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
    setHasSkippedQuickSearchSetup(false);
    if (typeof window === "undefined") {
      return;
    }
    try {
      window.localStorage.setItem(initialModelSetupCompletedKey, "1");
      window.localStorage.removeItem(modelSetupHintDismissedKey);
      window.localStorage.removeItem(QUICK_SEARCH_SETUP_SKIPPED_KEY);
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

  const actions = useQuickSetupActions({
    canDismissQuickModelSetup,
    defaultProvider,
    hasSkippedQuickFeishuSetup,
    hasSkippedQuickSearchSetup,
    initialModelSetupCompletedKey,
    isBlockingInitialModelSetup,
    isQuickSetupBusy,
    loadModels,
    loadSearchConfigs,
    modelSetupHintDismissedKey,
    modelCount: models.length,
    openSettingsAtTab,
    quickModelForm,
    quickModelSaving,
    quickModelTesting,
    quickSearchForm,
    quickSearchSaving,
    quickSearchTesting,
    resetQuickSetupUiState,
    searchConfigCount: searchConfigs.length,
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
  });

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

export type QuickSetupCoordinatorState = ReturnType<typeof useQuickSetupCoordinator>;
