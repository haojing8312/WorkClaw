# Contributing to WorkClaw

Thank you for contributing to WorkClaw.

## Before You Start

- Read [README.md](README.md) for project context and setup basics.
- Review open issues before starting new work to avoid duplicate efforts.
- For behavior expectations, see [CODE_OF_CONDUCT.md](CODE_OF_CONDUCT.md).

## Development Setup

```bash
pnpm install
pnpm app
```

For backend tests:

```bash
cd apps/runtime/src-tauri
cargo test
```

## How to Contribute

- Bug reports: open a GitHub issue with reproduction steps.
- Feature proposals: open a GitHub issue with use case and expected behavior.
- Code contributions: fork, create a branch, submit a pull request.
- Documentation improvements: PRs are welcome and usually the fastest to review.

## Pull Request Guidelines

- Keep PR scope focused; split unrelated changes into separate PRs.
- Add or update tests when behavior changes.
- Update docs when user-facing behavior changes.
- Use clear commit messages (Conventional Commits preferred, but not mandatory).
- Ensure the branch is up to date with `main` before requesting review.

## PR Checklist

- [ ] Code builds and tests pass locally.
- [ ] Documentation is updated when needed.
- [ ] No sensitive information (keys, tokens, credentials) is committed.
- [ ] Changes are limited to the intended scope.

## Security

Please do not disclose vulnerabilities in public issues.
See [SECURITY.md](SECURITY.md) for private reporting instructions.
