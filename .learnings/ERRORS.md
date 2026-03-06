## [ERR-20260306-001] git-commit-cmd-quoting

**Logged**: 2026-03-06T00:00:00+08:00
**Priority**: medium
**Status**: resolved
**Area**: config

### Summary
`git commit -m` with multiple quoted messages was misparsed in this `cmd` shell context.

### Error
```
error: pathspec 'shorten' did not match any file(s) known to git
error: pathspec 'architecture' did not match any file(s) known to git
error: pathspec 'section"' did not match any file(s) known to git
```

### Context
- Command attempted: `git commit -m "docs(readme): shorten architecture section" -m "..."`
- Environment: Codex shell running with `cmd`
- Result: quoted arguments were split and treated as pathspecs

### Suggested Fix
Use `git commit -F <message-file>` in this environment when a commit message contains spaces or multiple paragraphs.

### Metadata
- Reproducible: unknown
- Related Files: README.md, README.en.md

### Resolution
- **Resolved**: 2026-03-06T00:00:00+08:00
- **Commit/PR**: 2615d70
- **Notes**: Switched to `git commit -F <message-file>` for multi-line commit messages in this shell context.

---
