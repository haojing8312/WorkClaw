import { describe, expect, it } from "vitest";
import { createFeishuSetupState, transitionFeishuSetup } from "../feishu-setup";

describe("feishu setup states", () => {
  it("starts at INIT and can move to LOGIN_REQUIRED", () => {
    const state = createFeishuSetupState();
    expect(state.step).toBe("INIT");

    const next = transitionFeishuSetup(state, { type: "login.required" });
    expect(next.step).toBe("LOGIN_REQUIRED");
  });
});
