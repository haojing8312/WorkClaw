import { installDiagnosticsHandlers } from "../diagnostics";
import { invoke } from "@tauri-apps/api/core";

vi.mock("@tauri-apps/api/core", () => ({
  invoke: vi.fn(() => Promise.resolve(null)),
}));

describe("diagnostics bootstrap", () => {
  beforeEach(() => {
    vi.mocked(invoke).mockClear();
  });

  test("reports window errors to backend diagnostics command", async () => {
    installDiagnosticsHandlers();

    window.onerror?.("boom", "app.tsx", 12, 9, new Error("boom"));

    await Promise.resolve();

    expect(invoke).toHaveBeenCalledWith("record_frontend_diagnostic_event", {
      payload: expect.objectContaining({
        kind: "window_error",
        message: "boom",
        source: "app.tsx",
        line: 12,
        column: 9,
      }),
    });
  });
});
