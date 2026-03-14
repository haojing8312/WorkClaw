# Quick Model Footer Feedback Design

**Problem**

In the quick model setup dialog, users may not notice the result of clicking `测试连接` if the status message appears outside the current viewport. Even when the connection succeeds, the interaction can feel like it did nothing.

**Scope**

This change is intentionally minimal:

- Only improve result visibility in the quick model setup dialog
- Only adjust the footer action area feedback
- Do not redesign the rest of the form
- Do not add auto-scroll, toast notifications, or multi-location status messages

**Chosen Design**

Render a single status banner directly above the footer action buttons in the quick model setup dialog.

**Behavior**

- Clicking `测试连接` switches the button into a loading state (`测试中...`) and prevents repeated clicks until the request completes.
- On success, show a green footer banner with `连接成功，可直接保存并开始`.
- On failure, show a warning/error footer banner with `连接失败，请检查后重试`.
- The footer banner is the only connection-result banner in the dialog.

**State Rules**

- Only show the most recent test result.
- If the user edits a key connection field after a successful test, the previous success state should be cleared so stale validation is not implied.
- Detailed error text may still exist elsewhere if already supported, but the footer banner is the primary immediate feedback.

**Why This Design**

- It places feedback exactly where the user’s attention is after clicking the button.
- It avoids scroll-dependent discovery.
- It minimizes code churn while solving the perception problem.

**Validation**

- Verify the footer banner appears after clicking `测试连接`.
- Verify both success and failure states render in the footer.
- Verify editing a key field clears the previous test result.
