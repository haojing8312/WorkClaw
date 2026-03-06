# Desktop Lifecycle Design

**Date:** 2026-03-06

**Scope:** Desktop-only lifecycle capabilities for WorkClaw:
- Automatic updates
- Uninstall and local data handling
- Release/distribution UX

**Out of scope:**
- Core agent product flows
- Multi-agent orchestration UX
- IM or Feishu conversation features

---

## Goal

Make the Windows desktop app feel maintainable and trustworthy after installation:
- ordinary users can update in-app without going to GitHub manually
- enterprise users still have a stable MSI distribution path
- uninstall behavior is understandable and does not accidentally destroy user work
- release assets are easier to choose correctly

---

## Current State

### Already implemented

- Tag-driven Windows release workflow exists in [release-windows.yml](/e:/code/yzpd/skillhub/.github/workflows/release-windows.yml)
- Installer branding and Simplified Chinese installer UI are already configured in [tauri.conf.json](/e:/code/yzpd/skillhub/apps/runtime/src-tauri/tauri.conf.json)
- Runtime preferences already persist several app-level settings in [runtime_preferences.rs](/e:/code/yzpd/skillhub/apps/runtime/src-tauri/src/commands/runtime_preferences.rs)
- App data is already stored under `app_data_dir()` in multiple backend flows

### Gaps

- No Tauri updater plugin in [Cargo.toml](/e:/code/yzpd/skillhub/apps/runtime/src-tauri/Cargo.toml)
- No updater plugin initialization in [lib.rs](/e:/code/yzpd/skillhub/apps/runtime/src-tauri/src/lib.rs)
- No updater UI in settings or homepage
- No update metadata/signing flow in GitHub Release automation
- No clear uninstall/data-management surface for users
- Release page copy is too generic and does not distinguish `.exe` vs `.msi`

---

## Decision Summary

### 1. Auto update source

Use `GitHub Releases + Tauri Updater`.

Reason:
- no new server is required
- the project already uses GitHub Releases
- it keeps the first version operationally simple

### 2. Update package choice

Use `NSIS .exe` as the in-app update target.

Reason:
- better fit for consumer self-update flow
- MSI remains available for enterprise deployment
- avoids mixing enterprise deployment expectations into consumer update behavior

### 3. MSI positioning

Keep `.msi` as a manual installation/deployment artifact only.

Reason:
- enterprise IT teams often expect MSI for controlled rollout
- application self-update should stay simple and consistent

### 4. Uninstall strategy

Phase 1 does **not** add a custom uninstall wizard.

Instead:
- show data locations clearly in-app
- let users clean cache/logs before uninstall
- explicitly state what uninstall does and does not remove

Reason:
- lower implementation risk
- avoids destructive uninstall choices
- keeps maintenance cost down

---

## User Experience Design

## A. Automatic Update

### Entry points

Two entry points are enough:
- Settings page: authoritative update controls
- Lightweight banner/card on home screen after a new version is detected

Do not add more entry points in phase 1.

### User flow

1. App starts
2. After a short delay, app performs a background update check
3. If no update exists, no interruption
4. If an update exists, show version card with:
   - version number
   - publish date
   - short release summary
   - `稍后提醒我`
   - `下载并安装`
5. User chooses install
6. App downloads update package
7. App switches to `ready to install`
8. User confirms restart/install
9. Installer runs and app restarts into the new version

### Visible states

- `正在检查更新`
- `当前已是最新版本`
- `发现新版本 vX.Y.Z`
- `正在下载更新`
- `下载完成，准备安装`
- `更新失败`

### Dismiss behavior

If the user dismisses a discovered version:
- suppress reminders for that version for 24 hours
- allow manual re-check anytime

### Failure handling

- Network failure: show `暂时无法连接更新服务`
- Missing updater metadata: show `当前版本暂不支持自动更新，请前往 Release 页面下载`
- Signature verification failure: show a strong warning and block install
- Download interrupted: allow retry
- If current install was from MSI: show `检测到企业安装模式，请下载新安装包升级`

---

## B. Uninstall and Data Handling

### User problem to solve

Users need to understand:
- what uninstall removes
- what uninstall keeps
- where their data lives

### Phase 1 in-app surface

Add a `数据与卸载` section in Settings:
- Application data directory
- Cache/log directory
- Default workspace directory

For each path:
- show path
- `打开目录`

Provide actions:
- `清理缓存与日志`
- `导出环境摘要`

### Explicit behavior statement

The UI must state:
- uninstall removes the app program
- uninstall does not delete the workspace by default
- conversation data, memory, and local app data may remain unless manually removed

### Why not custom uninstall now

Custom NSIS/WiX uninstall options add:
- more QA burden
- more regression risk
- higher chance of data loss mistakes

That is not justified in phase 1.

---

## C. Release and Distribution UX

### Release asset positioning

Every release should clearly separate:
- `.exe`: recommended for most users
- `.msi`: enterprise/IT deployment

### Release body structure

Use a template-based release body with:
- Highlights
- Recommended download
- Enterprise deployment note
- Auto update note
- Known issues
- Data retention note

### Release assets to publish

- NSIS installer `.exe`
- MSI installer `.msi`
- updater signature files
- updater metadata `latest.json`
- checksums file

### Why this matters

Right now users have to infer which installer to choose.
That creates unnecessary support burden and weakens confidence in future auto-update behavior.

---

## Architecture Design

## A. Backend / Tauri layer

### New dependencies

Add Tauri updater plugin on the Rust side and JS side.

Rust side:
- plugin registration in app bootstrap

Frontend side:
- updater API wrapper
- app-level hook/service for check/download/install status

### Tauri config

Extend [tauri.conf.json](/e:/code/yzpd/skillhub/apps/runtime/src-tauri/tauri.conf.json) with updater configuration:
- updater endpoints
- public key
- Windows install mode tuned for interactive but low-friction update

Recommended Windows mode:
- `passive`

Reason:
- user still sees progress
- less surprising than silent install
- less noisy than full manual flow

## B. Runtime preferences

Extend runtime preferences with:
- `auto_update_enabled`
- `update_channel`
- `dismissed_update_version`
- `last_update_check_at`

Default values:
- `auto_update_enabled = true`
- `update_channel = stable`

No user-facing multi-channel complexity in phase 1 beyond a stored field.

## C. Frontend structure

Suggested additions:
- `apps/runtime/src/lib/updater.ts`
- `apps/runtime/src/hooks/useAppUpdater.ts`
- integrate into [SettingsView.tsx](/e:/code/yzpd/skillhub/apps/runtime/src/components/SettingsView.tsx)

Do not introduce a large new page just for updates.

---

## State Machine

```text
idle
  -> checking
checking
  -> up_to_date
  -> update_available
  -> failed
update_available
  -> deferred
  -> downloading
downloading
  -> ready_to_install
  -> failed
ready_to_install
  -> installing
  -> deferred
installing
  -> restart_required
  -> failed
deferred
  -> checking
failed
  -> checking
restart_required
  -> terminal until app relaunch
```

Rules:
- only one update check at a time
- only one download/install at a time
- manual check can override cooldown suppression

---

## Failure Matrix

| Scenario | User-visible result | Recovery |
| --- | --- | --- |
| GitHub unreachable | Update check failed | Retry later or manual download |
| `latest.json` missing | Auto update unavailable | Open release page |
| Signature invalid | Block install | Manual download after maintainer fix |
| User on MSI install | Manual upgrade notice | Download latest installer |
| Download interrupted | Retry CTA | Resume from update card |
| Install launch failed | Install failed message | Retry or manual download |

---

## Security and Operations

### Maintainer-only setup

Maintainers must configure:
- updater signing key pair
- GitHub Actions secrets for private key and password
- release workflow generation of updater metadata/signatures

End users do not configure these items.

### Key rotation note

If signing key is rotated:
- old clients may stop trusting new updates unless migration is planned
- this requires explicit maintainer documentation

---

## Testing Strategy

### Rust / config

- updater plugin initialization test coverage where practical
- config validation for updater fields

### Frontend

- unit tests for updater state transitions
- settings UI tests for:
  - latest version state
  - update available state
  - download/install state
  - failure state
  - data-and-uninstall section rendering

### Release workflow

- validate version + tag
- validate updater config presence
- dry-run checks for generated metadata files if feasible

### Manual verification

- install current build with NSIS
- simulate update from previous released version
- verify MSI path shows manual upgrade guidance
- verify uninstall guidance matches actual filesystem behavior

---

## Implementation Phasing

### Phase 1

- GitHub Releases based updater
- Settings-based update UI
- release metadata/signing integration
- data/uninstall settings section
- release body improvements

### Phase 2

- custom uninstall options
- update channels beyond `stable`
- enterprise/private update source abstraction

---

## Success Criteria

- Users on standard `.exe` installs can update from inside the app
- Users on `.msi` installs are clearly guided to manual upgrade
- Release page makes `.exe` vs `.msi` choice obvious
- Settings page clearly explains data locations and uninstall implications
- Update failures degrade gracefully and never block normal app usage
