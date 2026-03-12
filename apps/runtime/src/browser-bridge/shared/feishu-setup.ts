export type FeishuSetupStep = "INIT" | "LOGIN_REQUIRED";

export interface FeishuSetupState {
  step: FeishuSetupStep;
}

export type FeishuSetupEvent = { type: "login.required" };

export function createFeishuSetupState(): FeishuSetupState {
  return { step: "INIT" };
}

export function transitionFeishuSetup(
  state: FeishuSetupState,
  event: FeishuSetupEvent,
): FeishuSetupState {
  if (state.step === "INIT" && event.type === "login.required") {
    return { step: "LOGIN_REQUIRED" };
  }

  return state;
}
