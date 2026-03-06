import {
  check,
  type DownloadEvent,
  type Update,
} from "@tauri-apps/plugin-updater";

export interface AvailableAppUpdate {
  currentVersion: string;
  version: string;
  date?: string;
  body?: string;
  rawJson: Record<string, unknown>;
  nativeUpdate: Update;
}

export interface AppUpdateDownloadProgress {
  contentLength: number | null;
  downloadedBytes: number;
  percent: number | null;
}

function mapUpdate(update: Update): AvailableAppUpdate {
  return {
    currentVersion: update.currentVersion,
    version: update.version,
    date: update.date,
    body: update.body,
    rawJson: update.rawJson,
    nativeUpdate: update,
  };
}

export async function checkForAppUpdate(): Promise<AvailableAppUpdate | null> {
  const update = await check();
  return update ? mapUpdate(update) : null;
}

export async function downloadAppUpdate(
  update: AvailableAppUpdate,
  onProgress?: (progress: AppUpdateDownloadProgress) => void,
): Promise<void> {
  let contentLength: number | null = null;
  let downloadedBytes = 0;
  await update.nativeUpdate.download((event: DownloadEvent) => {
    if (event.event === "Started") {
      contentLength = typeof event.data.contentLength === "number" ? event.data.contentLength : null;
    } else if (event.event === "Progress") {
      downloadedBytes += event.data.chunkLength;
    }
    const percent =
      contentLength && contentLength > 0
        ? Math.min(100, Math.round((downloadedBytes / contentLength) * 100))
        : event.event === "Finished"
          ? 100
          : null;
    onProgress?.({
      contentLength,
      downloadedBytes,
      percent,
    });
  });
}

export async function installAppUpdate(update: AvailableAppUpdate): Promise<void> {
  await update.nativeUpdate.install();
}
