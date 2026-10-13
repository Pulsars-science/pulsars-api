## What and why

<!-- What this pull request changes, and why. Link the issue. -->

## Checklist

- [ ] Every commit is signed off (`git commit -s`)
- [ ] Follows `docs/contract/` (or the contract change is merged first)
- [ ] Tests added or updated; `cargo test` passes against PostgreSQL and Redis
- [ ] `cargo fmt`, `cargo clippy -- -D warnings` and `cargo deny check` pass
- [ ] No secret, no `unwrap()`/`expect()` on a request path
- [ ] New settings documented in `.env.example`
- [ ] `CHANGELOG.md` updated for user-visible changes
