# Support

## Get Help

- Usage questions and troubleshooting:
  Open a GitHub issue in this repository.
- Feature requests:
  Open a GitHub issue and describe the business/use-case context.
- Security reports:
  Follow [SECURITY.md](SECURITY.md) and use private disclosure.

## Source Build Issues

For Windows source-build or local setup failures, include enough toolchain evidence to separate environment problems from project defects.

Start here:

- [Windows 开发环境排障](docs/troubleshooting/windows-dev-setup.md)

Before filing a Windows source-build issue, collect:

- The exact failing command
- The relevant error excerpt
- `node -v`
- `pnpm -v`
- `rustc -vV`
- `rustup show`
- `where link`
- Whether you use Visual Studio stable or Preview/Insiders
- `pnpm doctor:windows`

## Issue Quality Checklist

When opening an issue, include:

- What you expected to happen
- What actually happened
- Steps to reproduce
- Environment details (OS, architecture, app version/commit, model/provider settings if relevant)
- Relevant logs or screenshots (with sensitive data removed)

## Response Expectations

This project is actively developed, but response times can vary by issue complexity and maintainer availability.

High-impact bugs and security issues are prioritized.
