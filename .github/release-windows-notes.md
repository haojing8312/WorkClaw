## WorkClaw Windows Release

- Recommended download: `*-setup.exe` for direct install and in-app auto-update.
- Enterprise deployment: `*.msi` for IT-managed installation and manual upgrades.
- Auto-update metadata: `latest.json` and `.sig` files are uploaded for desktop updater clients.

## Installation Guide

1. Most users should install the `setup.exe` package.
2. Enterprise or managed devices can use the `.msi` package.
3. Existing desktop installs configured for auto-update will consume the NSIS updater artifacts.

## Verification Checklist

- Installer branding and Chinese setup wizard verified.
- Release tag matches desktop app version.
- Updater signatures generated from release signing key.
