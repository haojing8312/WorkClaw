import { useCallback, useEffect, useMemo, useRef, useState } from "react";
import { RuntimePreferences } from "../types";
import {
  AppUpdateDownloadProgress,
  AvailableAppUpdate,
  checkForAppUpdate,
  downloadAppUpdate,
  installAppUpdate,
} from "../lib/updater";

export type AppUpdaterStatus =
  | "idle"
  | "checking"
  | "up_to_date"
  | "update_available"
  | "downloading"
  | "ready_to_install"
  | "installing"
  | "restart_required"
  | "deferred"
  | "failed";

type UpdaterPreferencesPatch = Partial<
  Pick<RuntimePreferences, "dismissed_update_version" | "last_update_check_at">
>;

interface CheckForUpdatesOptions {
  manual?: boolean;
}

interface UseAppUpdaterOptions {
  enabled?: boolean;
  autoCheck?: boolean;
  startupDelayMs?: number;
  dismissCooldownMs?: number;
  dismissedVersion?: string;
  lastCheckedAt?: string;
  onPreferencesChange?: (patch: UpdaterPreferencesPatch) => void | Promise<void>;
  now?: () => Date;
}

const DEFAULT_STARTUP_DELAY_MS = 10_000;
const DEFAULT_DISMISS_COOLDOWN_MS = 24 * 60 * 60 * 1000;

function normalizeText(value: string | undefined): string {
  return typeof value === "string" ? value.trim() : "";
}

function isBusyStatus(status: AppUpdaterStatus) {
  return status === "checking" || status === "downloading" || status === "installing";
}

function shouldSuppressVersionReminder(args: {
  updateVersion: string;
  manual: boolean;
  dismissedVersion: string;
  lastCheckedAt: string;
  dismissCooldownMs: number;
  now: Date;
}) {
  if (args.manual) return false;
  if (!args.dismissedVersion || args.dismissedVersion !== args.updateVersion) return false;
  const lastCheckedMs = Date.parse(args.lastCheckedAt);
  if (!Number.isFinite(lastCheckedMs)) return false;
  return args.now.getTime() - lastCheckedMs < args.dismissCooldownMs;
}

export function useAppUpdater(options?: UseAppUpdaterOptions) {
  const enabled = options?.enabled ?? true;
  const autoCheck = options?.autoCheck ?? true;
  const startupDelayMs = Math.max(0, options?.startupDelayMs ?? DEFAULT_STARTUP_DELAY_MS);
  const dismissCooldownMs = Math.max(
    0,
    options?.dismissCooldownMs ?? DEFAULT_DISMISS_COOLDOWN_MS,
  );
  const getNow = options?.now ?? (() => new Date());

  const [status, setStatus] = useState<AppUpdaterStatus>("idle");
  const [error, setError] = useState("");
  const [availableUpdate, setAvailableUpdate] = useState<AvailableAppUpdate | null>(null);
  const [downloadProgress, setDownloadProgress] = useState<AppUpdateDownloadProgress>({
    contentLength: null,
    downloadedBytes: 0,
    percent: null,
  });
  const [dismissedVersionState, setDismissedVersionState] = useState(() =>
    normalizeText(options?.dismissedVersion),
  );
  const [lastCheckedAtState, setLastCheckedAtState] = useState(() =>
    normalizeText(options?.lastCheckedAt),
  );

  const mountedRef = useRef(true);
  const hasCheckedRef = useRef(false);

  useEffect(() => {
    mountedRef.current = true;
    return () => {
      mountedRef.current = false;
    };
  }, []);

  useEffect(() => {
    setDismissedVersionState(normalizeText(options?.dismissedVersion));
  }, [options?.dismissedVersion]);

  useEffect(() => {
    setLastCheckedAtState(normalizeText(options?.lastCheckedAt));
  }, [options?.lastCheckedAt]);

  const persistPreferencesPatch = useCallback(
    async (patch: UpdaterPreferencesPatch) => {
      if (!options?.onPreferencesChange) return;
      try {
        await options.onPreferencesChange(patch);
      } catch (persistError) {
        console.warn("persist updater preferences failed", persistError);
      }
    },
    [options],
  );

  const resetDownloadProgress = useCallback(() => {
    setDownloadProgress({
      contentLength: null,
      downloadedBytes: 0,
      percent: null,
    });
  }, []);

  const checkForUpdates = useCallback(
    async (checkOptions?: CheckForUpdatesOptions) => {
      if (isBusyStatus(status)) {
        return availableUpdate;
      }

      const manual = Boolean(checkOptions?.manual);
      setStatus("checking");
      setError("");
      resetDownloadProgress();
      hasCheckedRef.current = true;

      try {
        const now = getNow();
        const checkedAt = now.toISOString();
        const nextUpdate = await checkForAppUpdate();
        if (!mountedRef.current) return nextUpdate;

        setLastCheckedAtState(checkedAt);
        await persistPreferencesPatch({ last_update_check_at: checkedAt });

        if (!nextUpdate) {
          setAvailableUpdate(null);
          setStatus("up_to_date");
          return null;
        }

        setAvailableUpdate(nextUpdate);
        if (
          shouldSuppressVersionReminder({
            updateVersion: nextUpdate.version,
            manual,
            dismissedVersion: dismissedVersionState,
            lastCheckedAt: lastCheckedAtState || checkedAt,
            dismissCooldownMs,
            now,
          })
        ) {
          setStatus("deferred");
          return nextUpdate;
        }

        setStatus("update_available");
        return nextUpdate;
      } catch (checkError) {
        if (!mountedRef.current) return null;
        setAvailableUpdate(null);
        setStatus("failed");
        setError(checkError instanceof Error ? checkError.message : String(checkError ?? "检查更新失败"));
        return null;
      }
    },
    [
      availableUpdate,
      dismissCooldownMs,
      dismissedVersionState,
      getNow,
      lastCheckedAtState,
      persistPreferencesPatch,
      resetDownloadProgress,
      status,
    ],
  );

  useEffect(() => {
    if (!enabled || !autoCheck || hasCheckedRef.current) return;
    const timer = window.setTimeout(() => {
      if (hasCheckedRef.current) return;
      void checkForUpdates();
    }, startupDelayMs);
    return () => {
      window.clearTimeout(timer);
    };
  }, [autoCheck, checkForUpdates, enabled, startupDelayMs]);

  const dismissUpdate = useCallback(() => {
    if (!availableUpdate) return;
    const dismissedAt = getNow().toISOString();
    setDismissedVersionState(availableUpdate.version);
    setLastCheckedAtState(dismissedAt);
    setStatus("deferred");
    setError("");
    void persistPreferencesPatch({
      dismissed_update_version: availableUpdate.version,
      last_update_check_at: dismissedAt,
    });
  }, [availableUpdate, getNow, persistPreferencesPatch]);

  const downloadUpdate = useCallback(async () => {
    if (!availableUpdate) {
      setStatus("failed");
      setError("当前没有可下载的更新");
      return;
    }

    setStatus("downloading");
    setError("");
    resetDownloadProgress();

    try {
      await downloadAppUpdate(availableUpdate, (progress) => {
        if (!mountedRef.current) return;
        setDownloadProgress(progress);
      });
      if (!mountedRef.current) return;
      setStatus("ready_to_install");
      setDownloadProgress((current) => ({
        contentLength: current.contentLength,
        downloadedBytes: current.downloadedBytes,
        percent: current.percent ?? 100,
      }));
    } catch (downloadError) {
      if (!mountedRef.current) return;
      setStatus("failed");
      setError(
        downloadError instanceof Error
          ? downloadError.message
          : String(downloadError ?? "下载更新失败"),
      );
    }
  }, [availableUpdate, resetDownloadProgress]);

  const installUpdate = useCallback(async () => {
    if (!availableUpdate || status !== "ready_to_install") {
      setStatus("failed");
      setError("更新尚未下载完成");
      return;
    }

    setStatus("installing");
    setError("");
    try {
      await installAppUpdate(availableUpdate);
      if (!mountedRef.current) return;
      setStatus("restart_required");
    } catch (installError) {
      if (!mountedRef.current) return;
      setStatus("failed");
      setError(
        installError instanceof Error
          ? installError.message
          : String(installError ?? "安装更新失败"),
      );
    }
  }, [availableUpdate, status]);

  const resetFailure = useCallback(() => {
    setError("");
    if (!availableUpdate) {
      setStatus("idle");
      return;
    }
    if (downloadProgress.percent === 100) {
      setStatus("ready_to_install");
      return;
    }
    setStatus("update_available");
  }, [availableUpdate, downloadProgress.percent]);

  const canDismiss = status === "update_available" || status === "deferred";
  const canDownload = status === "update_available";
  const canInstall = status === "ready_to_install";
  const isWorking = isBusyStatus(status);

  return useMemo(
    () => ({
      status,
      error,
      availableUpdate,
      downloadProgress,
      dismissedVersion: dismissedVersionState,
      lastCheckedAt: lastCheckedAtState,
      isWorking,
      canDismiss,
      canDownload,
      canInstall,
      checkForUpdates,
      dismissUpdate,
      downloadUpdate,
      installUpdate,
      resetFailure,
    }),
    [
      availableUpdate,
      canDismiss,
      canDownload,
      canInstall,
      checkForUpdates,
      dismissUpdate,
      dismissedVersionState,
      downloadProgress,
      downloadUpdate,
      error,
      installUpdate,
      isWorking,
      lastCheckedAtState,
      resetFailure,
      status,
    ],
  );
}
