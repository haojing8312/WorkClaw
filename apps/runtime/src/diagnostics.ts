import { invoke } from "@tauri-apps/api/core";

type FrontendDiagnosticPayload = {
  kind: string;
  message: string;
  stack?: string;
  source?: string;
  line?: number;
  column?: number;
  href?: string;
};

let installed = false;

async function reportFrontendDiagnostic(payload: FrontendDiagnosticPayload) {
  try {
    await invoke("record_frontend_diagnostic_event", { payload });
  } catch {
    // Keep diagnostics best-effort only.
  }
}

export function installDiagnosticsHandlers() {
  if (installed || typeof window === "undefined") {
    return;
  }
  installed = true;

  window.onerror = (message, source, line, column, error) => {
    void reportFrontendDiagnostic({
      kind: "window_error",
      message: String(message ?? "Unknown window error"),
      stack: error instanceof Error ? error.stack : undefined,
      source: typeof source === "string" ? source : undefined,
      line: typeof line === "number" ? line : undefined,
      column: typeof column === "number" ? column : undefined,
      href: window.location?.href,
    });
    return false;
  };

  window.onunhandledrejection = (event) => {
    const reason = event.reason;
    void reportFrontendDiagnostic({
      kind: "unhandled_rejection",
      message:
        reason instanceof Error
          ? reason.message
          : typeof reason === "string"
            ? reason
            : "Unhandled promise rejection",
      stack: reason instanceof Error ? reason.stack : undefined,
      href: window.location?.href,
    });
  };
}
