import { act, renderHook, waitFor } from "@testing-library/react";
import { useAppUpdater } from "../useAppUpdater";

const checkMock = vi.fn();

vi.mock("@tauri-apps/plugin-updater", () => ({
  check: (...args: unknown[]) => checkMock(...args),
}));

function createDeferred<T>() {
  let resolve!: (value: T | PromiseLike<T>) => void;
  let reject!: (reason?: unknown) => void;
  const promise = new Promise<T>((res, rej) => {
    resolve = res;
    reject = rej;
  });
  return { promise, resolve, reject };
}

function createUpdate(version = "0.2.4") {
  return {
    currentVersion: "0.2.3",
    version,
    date: "2026-03-06T10:00:00.000Z",
    body: "Bug fixes and improvements",
    rawJson: { version },
    download: vi.fn(async () => undefined),
    install: vi.fn(async () => undefined),
  };
}

describe("useAppUpdater", () => {
  beforeEach(() => {
    checkMock.mockReset();
    vi.useRealTimers();
  });

  test("transitions from checking to update_available on manual check", async () => {
    const deferred = createDeferred<ReturnType<typeof createUpdate> | null>();
    checkMock.mockReturnValueOnce(deferred.promise);
    const onPreferencesChange = vi.fn();

    const { result } = renderHook(() =>
      useAppUpdater({
        autoCheck: false,
        onPreferencesChange,
      }),
    );

    act(() => {
      void result.current.checkForUpdates({ manual: true });
    });

    expect(result.current.status).toBe("checking");

    await act(async () => {
      deferred.resolve(createUpdate());
      await deferred.promise;
    });

    await waitFor(() => {
      expect(result.current.status).toBe("update_available");
    });
    expect(result.current.availableUpdate?.version).toBe("0.2.4");
    expect(onPreferencesChange).toHaveBeenCalledWith(
      expect.objectContaining({
        last_update_check_at: expect.any(String),
      }),
    );
  });

  test("runs startup check and settles on up_to_date when nothing is available", async () => {
    vi.useFakeTimers();
    checkMock.mockResolvedValueOnce(null);

    const { result } = renderHook(() =>
      useAppUpdater({
        enabled: true,
        autoCheck: true,
        startupDelayMs: 250,
      }),
    );

    expect(checkMock).not.toHaveBeenCalled();

    await act(async () => {
      vi.advanceTimersByTime(250);
      await Promise.resolve();
      await Promise.resolve();
    });

    expect(checkMock).toHaveBeenCalledTimes(1);
    expect(result.current.status).toBe("up_to_date");
  });

  test("supports dismissing an update and manual override for the same version", async () => {
    const onPreferencesChange = vi.fn();
    const { result } = renderHook(() =>
      useAppUpdater({
        autoCheck: false,
        onPreferencesChange,
      }),
    );

    checkMock.mockResolvedValueOnce(createUpdate("0.2.5"));
    await act(async () => {
      await result.current.checkForUpdates({ manual: true });
    });
    expect(result.current.status).toBe("update_available");

    act(() => {
      result.current.dismissUpdate();
    });

    expect(result.current.status).toBe("deferred");
    expect(onPreferencesChange).toHaveBeenLastCalledWith(
      expect.objectContaining({
        dismissed_update_version: "0.2.5",
        last_update_check_at: expect.any(String),
      }),
    );

    checkMock.mockResolvedValueOnce(createUpdate("0.2.5"));
    await act(async () => {
      await result.current.checkForUpdates();
    });
    expect(result.current.status).toBe("deferred");

    checkMock.mockResolvedValueOnce(createUpdate("0.2.5"));
    await act(async () => {
      await result.current.checkForUpdates({ manual: true });
    });
    expect(result.current.status).toBe("update_available");
  });

  test("downloads an available update and marks it ready to install", async () => {
    const update = createUpdate("0.2.6");
    let emitProgress: ((event: { event: string; data?: Record<string, number> }) => void) | null =
      null;
    const downloadDeferred = createDeferred<undefined>();
    update.download.mockImplementation((onEvent?: (event: unknown) => void) => {
      emitProgress = onEvent as (event: { event: string; data?: Record<string, number> }) => void;
      return downloadDeferred.promise;
    });

    checkMock.mockResolvedValueOnce(update);
    const { result } = renderHook(() => useAppUpdater({ autoCheck: false }));

    await act(async () => {
      await result.current.checkForUpdates({ manual: true });
    });

    act(() => {
      void result.current.downloadUpdate();
    });
    expect(result.current.status).toBe("downloading");

    act(() => {
      emitProgress?.({ event: "Started", data: { contentLength: 100 } });
      emitProgress?.({ event: "Progress", data: { chunkLength: 25 } });
      emitProgress?.({ event: "Progress", data: { chunkLength: 75 } });
      emitProgress?.({ event: "Finished" });
    });

    await act(async () => {
      downloadDeferred.resolve(undefined);
      await downloadDeferred.promise;
    });

    await waitFor(() => {
      expect(result.current.status).toBe("ready_to_install");
    });
    expect(result.current.downloadProgress.downloadedBytes).toBe(100);
    expect(result.current.downloadProgress.percent).toBe(100);
  });

  test("installs a downloaded update and transitions to restart_required", async () => {
    const update = createUpdate("0.2.7");
    checkMock.mockResolvedValueOnce(update);
    const { result } = renderHook(() => useAppUpdater({ autoCheck: false }));

    await act(async () => {
      await result.current.checkForUpdates({ manual: true });
    });
    await act(async () => {
      await result.current.downloadUpdate();
    });
    await act(async () => {
      await result.current.installUpdate();
    });

    expect(update.install).toHaveBeenCalledTimes(1);
    expect(result.current.status).toBe("restart_required");
  });

  test("captures failures and can reset back to idle", async () => {
    checkMock.mockRejectedValueOnce(new Error("network down"));
    const { result } = renderHook(() => useAppUpdater({ autoCheck: false }));

    await act(async () => {
      await result.current.checkForUpdates({ manual: true });
    });

    expect(result.current.status).toBe("failed");
    expect(result.current.error).toContain("network down");

    act(() => {
      result.current.resetFailure();
    });

    expect(result.current.status).toBe("idle");
    expect(result.current.error).toBe("");
  });
});
