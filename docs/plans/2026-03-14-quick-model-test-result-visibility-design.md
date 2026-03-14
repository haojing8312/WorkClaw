# Quick Model Test Result Visibility Design

**Problem**

In the quick model setup dialog, the success message after clicking `测试连接` is rendered above the action area inside the scrollable form. When the user is already positioned near the bottom of the dialog, the result can appear outside the current viewport. This makes the action feel ineffective even when the connection test succeeds.

**User Goal**

After clicking `测试连接`, users should get immediate and visible feedback without needing to scroll.

**Chosen Approach**

Move the connection test result message to the action area near the `测试连接` and `保存并继续` buttons.

**Why This Approach**

- It places the feedback exactly where the user’s attention is after clicking.
- It avoids auto-scrolling, which can feel jumpy or disorienting in a modal.
- It keeps the success/failure state visible while the user decides whether to save.

**Scope**

- Quick model setup dialog only.
- No behavior change to the underlying connection test command.
- No change to the settings page model connection form unless it shares the same problematic layout later.

**UI Behavior**

- When the test succeeds, show `连接成功，可直接保存并开始` above the bottom action buttons.
- When the test fails, show `连接失败，请检查后重试` in the same location with failure styling.
- Remove the earlier result banner from the scrollable form section to avoid duplicated feedback.

**Error Handling**

- Keep existing detailed error messaging if `quickModelError` is present.
- Preserve the current loading and disabled states for `测试连接` and `保存并继续`.

**Testing**

- Update the quick setup test to assert that the test result is rendered in the action area after clicking `测试连接`.
- Ensure the success text remains visible in the dialog after the async command resolves.
