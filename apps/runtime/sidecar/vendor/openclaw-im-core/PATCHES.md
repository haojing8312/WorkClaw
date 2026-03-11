# Local Patches

- none

## Policy

- Record each local patch with file path, reason, risk, and removal strategy.
- Keep patches constrained to the sidecar adapter boundary.
- Do not let vendored IM adapter code depend on WorkClaw business-layer modules.
- For WeCom patches specifically, prefer additive config/ABI shims and avoid protocol forks unless upstream compatibility requires them.
