# Git Workflow

## Branch Strategy

**No direct pushes to `main`.** All work happens on dev branches.

### Branch naming

```
dev/v0.4.1       # patch release work
dev/v0.5.0       # minor release work
dev/v1.0.0       # major release work
```

One branch per release. All features, fixes, and docs for that release
go into the same branch.

### Workflow

```
1. Cut branch      git checkout -b dev/v0.4.1 main
2. Develop          commit freely, run pre-commit hook on each
3. Verify           cargo fmt + clippy + test + cargo deny
4. Merge to main    git checkout main && git merge --no-ff dev/v0.4.1
5. Tag              git tag -a v0.4.1 -m "v0.4.1 — ..."
6. Push             git push origin main && git push origin v0.4.1
7. Release          triggered automatically by tag push (release.yml)
8. Cleanup          git branch -d dev/v0.4.1
```

### Rules

| Rule | Why |
|---|---|
| Never push directly to `main` | Main must always be releasable |
| One dev branch per release | Keeps scope contained |
| Pre-commit hook must pass on every commit | fmt + clippy + unit tests |
| Merge with `--no-ff` | Preserves branch history in log |
| Tag only on `main` after merge | Tags trigger release pipeline |
| Delete dev branch after merge | Keep branch list clean |

### What goes in a dev branch

Everything for that release version:
- Feature implementation
- Bug fixes
- Test additions
- Doc updates
- CHANGELOG entry
- Version bump in Cargo.toml (last commit before merge)

### Pre-merge checklist

Before merging `dev/vX.Y.Z` into `main`:

- [ ] `cargo fmt --check` passes
- [ ] `cargo clippy -- -D warnings` passes
- [ ] `cargo test` passes (unit + E2E)
- [ ] `cargo deny check` passes
- [ ] `cargo build --release` succeeds
- [ ] CHANGELOG.md has entry for this version
- [ ] Cargo.toml version bumped
- [ ] All acceptance criteria from PATCH-PLAN.md or ROADMAP.md met
- [ ] No `unwrap()` in non-test code
- [ ] Docs updated to reflect changes

### Quick reference

```bash
# Start work on v0.4.1
git checkout main && git pull
git checkout -b dev/v0.4.1

# ... develop, commit, test ...

# Ready to release
cargo fmt --check && cargo clippy -- -D warnings && cargo test && cargo deny check
# Update CHANGELOG.md, bump Cargo.toml version

# Merge and release
git checkout main && git pull
git merge --no-ff dev/v0.4.1
git tag -a v0.4.1 -m "v0.4.1 — description"
git push origin main && git push origin v0.4.1
git branch -d dev/v0.4.1
```
