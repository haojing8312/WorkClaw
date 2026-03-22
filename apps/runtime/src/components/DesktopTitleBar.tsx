import { useCallback, useEffect, useRef, useState, type MouseEvent as ReactMouseEvent } from "react";
import { cursorPosition, getCurrentWindow, PhysicalPosition } from "@tauri-apps/api/window";
import { Minus, Square, X } from "lucide-react";
import workclawLogo from "../assets/branding/workclaw-logo.png";
import { reportFrontendDiagnostic } from "../diagnostics";

function getDesktopWindow() {
  if (typeof window === "undefined") {
    return null;
  }
  try {
    return getCurrentWindow();
  } catch {
    return null;
  }
}

function RestoreWindowIcon({ className }: { className?: string }) {
  return (
    <svg viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="1.8" className={className} aria-hidden="true">
      <path d="M9 5h8a2 2 0 0 1 2 2v8" />
      <path d="M7 9h8a2 2 0 0 1 2 2v8H9a2 2 0 0 1-2-2z" />
    </svg>
  );
}

function clampTitlebarRatio(value: number): number {
  if (!Number.isFinite(value)) {
    return 0.5;
  }
  return Math.min(Math.max(value, 0.1), 0.9);
}

const TITLEBAR_DRAG_THRESHOLD_PX = 6;

export function DesktopTitleBar() {
  const [isWindowMaximized, setIsWindowMaximized] = useState(false);
  const pendingTitlebarDragRef = useRef<{
    clientX: number;
    clientY: number;
    dragRegion: HTMLDivElement;
  } | null>(null);

  useEffect(() => {
    const desktopWindow = getDesktopWindow();
    if (!desktopWindow) {
      return;
    }

    let cancelled = false;
    let detachResizeListener: (() => void) | null = null;
    const syncMaximizedState = async () => {
      try {
        const value = await desktopWindow.isMaximized();
        if (!cancelled) {
          setIsWindowMaximized(value);
        }
      } catch (error) {
        console.warn("Failed to read window maximized state", error);
      }
    };

    void syncMaximizedState();
    void desktopWindow.onResized(() => {
      void syncMaximizedState();
    }).then((unlisten) => {
      if (cancelled) {
        unlisten();
        return;
      }
      detachResizeListener = unlisten;
    }).catch((error) => {
      console.warn("Failed to subscribe to desktop window resize events", error);
    });

    return () => {
      cancelled = true;
      detachResizeListener?.();
    };
  }, []);

  const handleWindowAction = useCallback(async (action: "minimize" | "toggleMaximize" | "close") => {
    const desktopWindow = getDesktopWindow();
    if (!desktopWindow) {
      return;
    }
    try {
      if (action === "toggleMaximize") {
        const maximized = await desktopWindow.isMaximized();
        if (maximized) {
          await desktopWindow.unmaximize();
        } else {
          await desktopWindow.maximize();
        }
        setIsWindowMaximized(!maximized);
        return;
      }
      await desktopWindow[action]();
    } catch (error) {
      console.warn(`Failed to execute desktop window action: ${action}`, error);
      void reportFrontendDiagnostic({
        kind: "window_control_error",
        message: `Desktop window action failed: ${action}`,
        stack: error instanceof Error ? error.stack : undefined,
        href: typeof window !== "undefined" ? window.location?.href : undefined,
      });
    }
  }, []);

  const clearPendingTitlebarDrag = useCallback(() => {
    pendingTitlebarDragRef.current = null;
  }, []);

  const handleTitlebarDragStart = useCallback(async (clientX: number, clientY: number, dragRegion: HTMLDivElement) => {
    const desktopWindow = getDesktopWindow();
    if (!desktopWindow) {
      return;
    }

    if (!isWindowMaximized) {
      try {
        await desktopWindow.startDragging();
      } catch (error) {
        console.warn("Failed to start dragging desktop window", error);
        void reportFrontendDiagnostic({
          kind: "window_control_error",
          message: "Desktop window drag failed",
          stack: error instanceof Error ? error.stack : undefined,
          href: typeof window !== "undefined" ? window.location?.href : undefined,
        });
      }
      return;
    }

    const rect = dragRegion.getBoundingClientRect();
    const pointerRatio = clampTitlebarRatio((clientX - rect.left) / Math.max(rect.width, 1));
    const pointerOffsetY = Math.max(12, Math.min(clientY - rect.top, rect.height - 8));

    try {
      const [screenCursor, restoredSize] = await Promise.all([
        cursorPosition(),
        (async () => {
          await desktopWindow.unmaximize();
          return desktopWindow.outerSize();
        })(),
      ]);

      const nextX = Math.round(screenCursor.x - restoredSize.width * pointerRatio);
      const nextY = Math.round(screenCursor.y - Math.min(pointerOffsetY, restoredSize.height - 1));

      await desktopWindow.setPosition(new PhysicalPosition(nextX, nextY));
      setIsWindowMaximized(false);
      await desktopWindow.startDragging();
    } catch (error) {
      console.warn("Failed to restore and drag desktop window from maximized state", error);
      void reportFrontendDiagnostic({
        kind: "window_control_error",
        message: "Desktop window restore-drag failed",
        stack: error instanceof Error ? error.stack : undefined,
        href: typeof window !== "undefined" ? window.location?.href : undefined,
      });
    }
  }, [isWindowMaximized]);

  const handleTitlebarMouseDown = useCallback((event: ReactMouseEvent<HTMLDivElement>) => {
    if (event.button !== 0) {
      clearPendingTitlebarDrag();
      return;
    }

    pendingTitlebarDragRef.current = {
      clientX: event.clientX,
      clientY: event.clientY,
      dragRegion: event.currentTarget,
    };
  }, [clearPendingTitlebarDrag]);

  const handleTitlebarMouseMove = useCallback((event: ReactMouseEvent<HTMLDivElement>) => {
    const pendingDrag = pendingTitlebarDragRef.current;
    if (!pendingDrag || (event.buttons & 1) !== 1) {
      return;
    }

    const deltaX = event.clientX - pendingDrag.clientX;
    const deltaY = event.clientY - pendingDrag.clientY;
    if (Math.hypot(deltaX, deltaY) < TITLEBAR_DRAG_THRESHOLD_PX) {
      return;
    }

    clearPendingTitlebarDrag();
    void handleTitlebarDragStart(event.clientX, event.clientY, pendingDrag.dragRegion);
  }, [clearPendingTitlebarDrag, handleTitlebarDragStart]);

  const maximizeButtonLabel = isWindowMaximized ? "还原窗口" : "最大化窗口";

  return (
    <header
      data-testid="app-titlebar"
      className="sm-titlebar sm-divider flex h-11 items-center border-b px-3"
    >
      <div
        data-testid="app-titlebar-drag-region"
        className="flex min-w-0 flex-1 items-center gap-2.5 self-stretch pr-3"
        onMouseDown={(event) => {
          handleTitlebarMouseDown(event);
        }}
        onMouseMove={(event) => {
          handleTitlebarMouseMove(event);
        }}
        onMouseUp={() => {
          clearPendingTitlebarDrag();
        }}
        onDoubleClick={() => {
          clearPendingTitlebarDrag();
          void handleWindowAction("toggleMaximize");
        }}
      >
        <div className="flex h-7 w-7 items-center justify-center rounded-[10px] border border-[var(--sm-border)] bg-[var(--sm-surface)] shadow-[var(--sm-shadow-sm)]">
          <img src={workclawLogo} alt="" className="h-5 w-5 object-contain" />
        </div>
        <span className="text-[13px] font-medium tracking-[0.01em] text-[var(--sm-text-muted)]">WorkClaw</span>
        <div className="min-w-0 flex-1 self-stretch" />
      </div>
      <div className="flex items-center gap-1">
        <button
          type="button"
          aria-label="最小化窗口"
          className="sm-window-control"
          onClick={() => {
            void handleWindowAction("minimize");
          }}
        >
          <Minus className="h-3.5 w-3.5" />
        </button>
        <button
          type="button"
          aria-label={maximizeButtonLabel}
          className="sm-window-control"
          onClick={() => {
            void handleWindowAction("toggleMaximize");
          }}
        >
          {isWindowMaximized ? <RestoreWindowIcon className="h-3.5 w-3.5" /> : <Square className="h-3.5 w-3.5" />}
        </button>
        <button
          type="button"
          aria-label="关闭窗口"
          className="sm-window-control sm-window-control-danger"
          onClick={() => {
            void handleWindowAction("close");
          }}
        >
          <X className="h-3.5 w-3.5" />
        </button>
      </div>
    </header>
  );
}
