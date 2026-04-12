import { useCallback } from "react";
import { buildAppShellRenderProps, type BuildAppShellRenderPropsOptions } from "./buildAppShellRenderProps";
import type { QuickSetupCoordinatorState } from "./useQuickSetupCoordinator";
import { extractErrorMessage } from "../app-shell-utils";
import { openExternalUrl } from "../utils/openExternalUrl";

interface UseAppRenderPropsOptions extends Omit<
  BuildAppShellRenderPropsOptions,
  | "openExternalQuickModelLink"
  | "createNewTab"
  | "closeSettings"
  | "showQuickModelSetup"
  | "quickSetupStep"
  | "canDismissQuickModelSetup"
  | "isBlockingInitialModelSetup"
  | "quickModelApiKeyInputRef"
  | "quickModelApiKeyVisible"
  | "quickModelError"
  | "quickModelForm"
  | "quickModelPresetKey"
  | "quickModelSaving"
  | "quickModelTestDisplay"
  | "quickModelTestResult"
  | "quickModelTesting"
  | "quickSearchApiKeyVisible"
  | "quickSearchError"
  | "quickSearchForm"
  | "quickSearchSaving"
  | "quickSearchTestResult"
  | "quickSearchTesting"
  | "selectedQuickModelProvider"
  | "shouldShowQuickModelRawMessage"
  | "applyQuickModelPreset"
  | "applyQuickSearchPreset"
  | "closeQuickModelSetup"
  | "setQuickModelForm"
  | "setQuickModelApiKeyVisible"
  | "setQuickModelError"
  | "setQuickModelTestResult"
  | "setQuickSearchForm"
  | "setQuickSearchApiKeyVisible"
  | "setQuickSearchError"
  | "setQuickSearchTestResult"
  | "saveQuickModelSetup"
  | "saveQuickSearchSetup"
  | "skipQuickSearchSetup"
  | "skipQuickFeishuSetup"
  | "openQuickFeishuSetupFromDialog"
  | "testQuickModelSetupConnection"
  | "testQuickSearchSetupConnection"
> {
  quickSetup: QuickSetupCoordinatorState;
  navigate: (view: "start-task" | "experts" | "experts-new" | "packaging" | "employees") => void;
  onAfterCreateTab: () => void;
  onCloseSettings: () => Promise<void>;
}

export function useAppRenderProps(options: UseAppRenderPropsOptions) {
  const {
    navigate,
    onAfterCreateTab,
    onCloseSettings,
    quickSetup,
    ...renderOptions
  } = options;
  const {
    actions: {
      applyQuickModelPreset,
      applyQuickSearchPreset,
      closeQuickModelSetup,
      openQuickFeishuSetupFromDialog,
      saveQuickModelSetup,
      saveQuickSearchSetup,
      skipQuickFeishuSetup,
      skipQuickSearchSetup,
      testQuickModelSetupConnection,
      testQuickSearchSetupConnection,
    },
    canDismissQuickModelSetup,
    isBlockingInitialModelSetup,
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
    shouldShowQuickModelRawMessage,
    showQuickModelSetup,
  } = quickSetup;

  const openExternalQuickModelLink = useCallback(
    (url: string) => {
      openExternalUrl(url).catch((error) => {
        setQuickModelError(
          extractErrorMessage(error, "打开外部链接失败，请稍后重试"),
        );
      });
    },
    [setQuickModelError],
  );

  const createNewTab = useCallback(() => {
    onAfterCreateTab();
    navigate("start-task");
  }, [navigate, onAfterCreateTab]);

  return buildAppShellRenderProps({
    ...renderOptions,
    showQuickModelSetup,
    quickSetupStep,
    canDismissQuickModelSetup,
    isBlockingInitialModelSetup,
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
    selectedQuickModelProvider,
    shouldShowQuickModelRawMessage,
    applyQuickModelPreset,
    applyQuickSearchPreset,
    closeQuickModelSetup,
    setQuickModelError,
    setQuickModelForm,
    setQuickModelApiKeyVisible,
    setQuickModelTestResult,
    setQuickSearchForm,
    setQuickSearchApiKeyVisible,
    setQuickSearchError,
    setQuickSearchTestResult,
    saveQuickModelSetup,
    saveQuickSearchSetup,
    skipQuickSearchSetup,
    skipQuickFeishuSetup,
    openQuickFeishuSetupFromDialog,
    testQuickModelSetupConnection,
    testQuickSearchSetupConnection,
    openExternalQuickModelLink,
    createNewTab,
    closeSettings: onCloseSettings,
  });
}
