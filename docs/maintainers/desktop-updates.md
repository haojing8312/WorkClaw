# Desktop Updates and Release Workflow

This project uses GitHub Releases as the phase-1 update source for the Tauri desktop app.

## Updater Architecture

- Desktop app endpoint: `https://github.com/haojing8312/WorkClaw/releases/latest/download/latest.json`
- Recommended self-update package: Windows NSIS `*-setup.exe`
- Enterprise/manual deployment package: Windows `.msi`

The desktop app must have:

- `plugins.updater` configured in `apps/runtime/src-tauri/tauri.conf.json`
- `bundle.createUpdaterArtifacts: true`
- A valid updater public key in `tauri.conf.json`

## Required GitHub Secrets

Configure these repository secrets before pushing a release tag:

- `TAURI_SIGNING_PRIVATE_KEY`
- `TAURI_SIGNING_PRIVATE_KEY_PASSWORD`

`GITHUB_TOKEN` is provided automatically by GitHub Actions.

## Generate Signing Keys

Run this once on a secure maintainer machine:

```bash
pnpm --dir apps/runtime tauri signer generate -- -w ~/.tauri/workclaw.key
```

Store:

- Public key: commit into `apps/runtime/src-tauri/tauri.conf.json`
- Private key: store in GitHub secret `TAURI_SIGNING_PRIVATE_KEY`
- Password: store in GitHub secret `TAURI_SIGNING_PRIVATE_KEY_PASSWORD`

If you lose the private key, existing installed clients can no longer trust future auto-updates signed by a different key. In that case you must ship a manual reinstall path.

## Release Workflow

When a maintainer pushes `vX.Y.Z`:

1. Version consistency is validated.
2. `.github/workflows/release-windows.yml` builds the Windows bundles.
3. `tauri-apps/tauri-action` uploads release assets, updater signatures, and `latest.json`.
4. Release notes are loaded from `.github/release-windows-notes.md`.

## Local Verification

Run before tagging a release:

```bash
pnpm release:check-version vX.Y.Z
node --test scripts/check-updater-config.test.mjs
pnpm build:runtime
```

## Rotation Notes

- Rotate the updater key only when you are prepared to force a manual reinstall for existing users.
- Do not rewrite historical release tags to fix updater issues; prefer a new patch version.
