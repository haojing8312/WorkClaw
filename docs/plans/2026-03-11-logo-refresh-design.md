# WorkClaw Logo Refresh Design

## Goal

Use the designer-delivered WorkClaw logo set as the single source of truth and replace every user-facing logo asset in the repository that affects the app UI, desktop bundle icons, and README branding.

## Scope

- Replace desktop bundle icon assets under `apps/runtime/src-tauri/icons/`.
- Replace the in-app branding image at `apps/runtime/src/assets/branding/workclaw-logo.png`.
- Replace the README-rendered logo image at `docs/workclaw_logo_w.png`.
- Keep existing file names and references unchanged to minimize code and packaging risk.

## Non-Goals

- Do not redesign NSIS/WiX installer banner/sidebar BMP compositions in this pass.
- Do not rename asset paths or change Tauri configuration.
- Do not alter product copy, only the rendered image assets.

## Recommended Approach

Use the new designer assets from `temp/WorkClaw桌面图标/WorkClaw桌面图标` as the source material and overwrite the current tracked assets in place.

This is the lowest-risk path because:

- `tauri.conf.json` already points at stable file names like `icons/icon.ico`.
- React UI code already imports `workclaw-logo.png`.
- README files already embed `docs/workclaw_logo_w.png`.
- Replacing file contents avoids refactor churn and reduces the chance of missing references.

## Asset Mapping

- Windows `.ico` source sizes map directly to `apps/runtime/src-tauri/icons/icon.ico`.
- Transparent source art (`透明背景.png` / `矢量图.svg`) is used to regenerate PNG-based app icons and brand images.
- UI brand image and README logo image should use a transparent-background render to preserve flexibility across surfaces.

## Verification

- Confirm all existing logo references still point to the same paths.
- Confirm all expected icon files still exist after replacement.
- Spot-check image dimensions and file sizes for key bundle assets.
- Avoid claiming success without fresh file-system evidence.
