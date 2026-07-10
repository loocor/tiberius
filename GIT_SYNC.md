# Git Sync Guide

This file is the source of truth for how the Xuntuo Tiberius fork follows
`prisma/tiberius` while preserving SQL Server 2005 and stored-procedure support.

If a future session needs to sync upstream, change the driver, resolve a
conflict, publish a patch, or update Xuntuo, read this file first and follow it
exactly.

## Why This Fork Exists

Xuntuo connects customer-site Windows Server 2008 R2 hosts to legacy SQL Server
2005 databases. Upstream Tiberius 0.12.3 provides the correct asynchronous TDS
foundation, but Xuntuo requires behavior that is not available in upstream
`main`:

- TDS 7.2 operation with `tds73` disabled;
- structured protocol errors instead of panics on supported legacy paths;
- lossless `money` and `smallmoney` handling;
- legacy string, collation, row, numeric, and type metadata preservation;
- manifest-derived typed parameters that avoid index-breaking implicit casts;
- named stored-procedure RPC requests;
- input, output, and input/output procedure parameters;
- streamed result sets, output values, return status, and rows-affected tokens;
- connection-idle detection so an interrupted TDS stream is never returned to a pool.

The fork keeps these changes reviewable and testable without turning the Xuntuo
application repository into the source of truth for a third-party driver.

## Goals

- Keep the fork close to official Tiberius.
- Keep Xuntuo patches small, focused, and independently reviewable.
- Preserve SQL Server 2005, TDS 7.2, and legacy Windows compatibility.
- Upstream generally useful fixes whenever their scope fits the official project.
- Pin every Xuntuo build to a validated full fork commit.
- Make upstream synchronization reproducible for a future maintainer or agent.

## Non-Goals

- Do not turn Tiberius into an ORM or Xuntuo business-logic layer.
- Do not add Portal, HTTP, tenant, or application authorization concerns here.
- Do not enable `tds73` in the Xuntuo dependency profile.
- Do not use the fork branch as a place for unrelated upstream modernization.
- Do not rewrite published patch history merely to make it look linear.

## Repository Model

- `upstream/main`
  Official Prisma Tiberius source of truth.
- `origin/main`
  Fork mirror of official `upstream/main`.
- local `main`
  Local mirror branch; no Xuntuo patches belong here.
- `origin/xuntuo/sqlserver-2005`
  Published Xuntuo integration branch.
- local `xuntuo/sqlserver-2005`
  Working integration branch that Xuntuo consumes.
- `xuntuo/patch-*`
  Short-lived branches for new driver changes before they are merged into the
  integration branch.

Rules:

- Never develop Xuntuo patches on `main`.
- Never push to `upstream`.
- Never rebase or force-push `xuntuo/sqlserver-2005` after Xuntuo has consumed it.
- Merge official `main` into the integration branch so conflict resolution stays auditable.
- Use a clean branch from `upstream/main` for any upstream pull request.
- Keep application-specific behavior out of the driver.

## Current Baseline And Patch Stack

- Upstream repository: `https://github.com/prisma/tiberius.git`
- Fork repository: `https://github.com/loocor/tiberius.git`
- Upstream base: `a6b4fcdae0de5702427290b89f8d05bc51f3bcfa`
- Integration branch: `xuntuo/sqlserver-2005`
- Current validated commit: `78f3585c36f2ef3ea887c0f4c0ec1f1bde197a5f`

Initial Xuntuo commits:

1. `a524e44 fix(tds): harden legacy sql server support`
2. `c30d3e2 feat(rpc): execute named stored procedures`
3. `78f3585 test(rpc): cover xuntuo driver extensions`

## Required Local Configuration

The nested checkout at `apps/bridge/vendor/tiberius` is an independent Git
repository. Xuntuo ignores it and does not use a submodule or gitlink.

Configure the checkout with:

```bash
git remote add upstream https://github.com/prisma/tiberius.git
git config remote.pushDefault origin
git config branch.xuntuo/sqlserver-2005.pushRemote origin
git config push.default simple
git config rerere.enabled true
```

If `upstream` already exists, verify it instead of adding it again:

```bash
git remote -v
git branch -vv
git config --get-regexp '^(remote\.pushDefault|branch\.xuntuo/sqlserver-2005\.pushRemote|push\.default|rerere\.enabled)$'
```

Expected relationships:

- `main -> origin/main`
- `xuntuo/sqlserver-2005 -> origin/xuntuo/sqlserver-2005`
- `upstream -> prisma/tiberius`

## Standard Upstream Sync

### 1. Start From A Clean Checkout

```bash
git status --short
```

If output is not empty, commit the intended fork change or stop. Do not mix an
upstream sync with unrelated work.

### 2. Fetch Both Repositories

```bash
git fetch upstream
git fetch origin
```

### 3. Fast-Forward The Fork Mirror

```bash
git switch main
git merge --ff-only upstream/main
git push origin main
```

If `--ff-only` fails, stop. Fork `main` must not contain private patches or
independent history.

### 4. Merge Upstream Into The Patch Branch

```bash
git switch xuntuo/sqlserver-2005
git merge --no-ff main
```

Do not rebase the published integration branch. Xuntuo records exact fork SHAs,
so rewriting old commits would make dependency history misleading.

### 5. Resolve Conflicts Deliberately

Inspect every conflict:

```bash
git status
git diff --diff-filter=U
```

Resolution policy:

- Prefer upstream structure and APIs by default.
- Re-apply only the smallest patch needed to preserve the invariants below.
- Do not restore deleted upstream code wholesale merely because the old patch touched it.
- Keep protocol errors structured and sanitized.
- Preserve byte limits, token ordering, and connection-idle guarantees.

After resolving:

```bash
git add <resolved-files>
git commit
```

If the merge direction is wrong or the conflict set is not understood:

```bash
git merge --abort
```

### 6. Verify The Fork

Run formatting and the no-database Xuntuo extension tests with TDS 7.3 disabled:

```bash
cargo fmt --all -- --check
cargo test --no-default-features --features native-tls \
  --test procedure_api --test query_api
```

The upstream 0.12.3 tree currently triggers newer Rust 1.95 Clippy warnings in
untouched code when all warnings are denied. Do not mix broad upstream lint
modernization into a protocol sync. Clippy is enforced at the Xuntuo Bridge
integration boundary instead.

### 7. Publish The Integration Branch

```bash
git push origin xuntuo/sqlserver-2005
```

Never push this branch with `--force` or `--force-with-lease`.

## Updating Xuntuo

Xuntuo keeps this fork as an independent nested checkout at
`apps/bridge/vendor/tiberius`. The parent repository tracks neither the fork
files nor a submodule pointer.

After publishing and validating a new fork commit:

1. Checkout the new commit in the nested repository.
2. Update the pinned `ref` in Xuntuo `.github/workflows/bridge.yml`.
3. Update `apps/bridge/THIRD_PARTY_NOTICES.md`.
4. Update `docs/03-architecture/tiberius-fork-strategy.md`.
5. Run the complete Xuntuo Bridge verification matrix.

```bash
git -C apps/bridge/vendor/tiberius fetch origin
git -C apps/bridge/vendor/tiberius switch xuntuo/sqlserver-2005
git -C apps/bridge/vendor/tiberius pull --ff-only

cargo fmt --manifest-path apps/bridge/Cargo.toml --all -- --check
cargo check --manifest-path apps/bridge/Cargo.toml
cargo clippy --manifest-path apps/bridge/Cargo.toml --all-targets -- -D warnings
cargo clippy --manifest-path apps/bridge/Cargo.toml \
  --no-default-features --features default-restricted \
  --all-targets -- -D warnings
cargo test --manifest-path apps/bridge/Cargo.toml -- --test-threads=1
cargo test --manifest-path apps/bridge/Cargo.toml \
  --no-default-features --features default-restricted \
  -- --test-threads=1
```

The new fork revision is not accepted until Windows packaging and a real SQL
Server 2005 `--check` run also pass when the change affects protocol, TLS,
runtime, or value handling.

## Must-Keep Xuntuo Invariants

During every upstream sync, preserve behavior rather than blindly preserving
old file locations.

### 1. SQL Server 2005 And TDS 7.2

- Xuntuo compiles Tiberius with `default-features = false` and `native-tls`.
- `tds73` remains disabled.
- Known unsupported tokens and metadata fail with structured errors, not panics.
- Rejected encryption requirements fail closed.

### 2. Legacy Values And Metadata

- `money` and `smallmoney` remain lossless.
- Legacy `varchar` retains server collation behavior.
- Row width and metadata mismatches are visible protocol errors.
- Manifest-derived filter values retain the exact SQL type needed to avoid implicit column conversion.

### 3. Named Procedure RPC

- Procedure names are encoded as named RPC calls.
- Input, output, and input/output parameters preserve names, ordinals, flags, and explicit SQL types.
- Default collation is applied to textual RPC parameters when required.
- Portal cannot inject an arbitrary SQL type declaration.

### 4. Procedure Result Stream

- Result-set boundaries remain visible, including empty result sets.
- Output parameter values preserve RPC order.
- Signed return status and rows-affected tokens remain available.
- Missing tokens are not synthesized.
- Interrupted streams leave the connection non-idle so the pool discards it.

### 5. Test Coverage

- `tests/procedure_api.rs` remains a public API contract test.
- `tests/query_api.rs` remains a no-database compatibility test.
- Xuntuo Bridge tests remain the integration authority for SQL compilation,
  response encoding, limits, timeout handling, and pool discard behavior.

## Upstream Contribution Policy

When a fix is useful beyond Xuntuo:

```bash
git fetch upstream
git switch -c upstream/<topic> upstream/main
```

Re-implement or cherry-pick the smallest generally useful change, add upstream-
appropriate tests, and open a pull request against `prisma/tiberius:main`.

Do not open an upstream PR directly from `xuntuo/sqlserver-2005`; that branch may
contain application-specific APIs and multiple unrelated patches.

If upstream merges the fix:

1. Sync fork `main`.
2. Merge `main` into `xuntuo/sqlserver-2005`.
3. Remove only the now-redundant fork delta.
4. Re-run fork and Xuntuo validation.

## Common Pitfalls

### Syncing From Origin Instead Of Upstream

`origin` is the fork. Fetching or merging `origin/main` alone does not prove the
fork contains the latest official Tiberius commits. Always fetch `upstream`.

### Rebasing The Published Patch Branch

Rebase changes commit identities. Xuntuo documentation and CI pin full commit
SHAs, so rebasing the integration branch breaks its audit trail. Merge upstream
instead.

### Editing The Nested Repo But Committing Xuntuo

Xuntuo ignores this directory. A driver edit will never appear in the parent
`git status`. Always check both repositories:

```bash
git status --short
git -C apps/bridge/vendor/tiberius status --short
```

### Moving The Fork Without Updating CI

A local checkout can be newer than CI. Every accepted fork update must change
the pinned CI SHA and third-party notice in the same Xuntuo commit.

### Treating Network Placement As Authentication

This fork controls TDS behavior only. It does not authenticate Portal-to-Bridge
HTTP requests. Data API machine authentication belongs in Xuntuo Bridge.

## Recovery

If the nested checkout is missing:

```bash
git clone --branch xuntuo/sqlserver-2005 --single-branch \
  https://github.com/loocor/tiberius.git \
  apps/bridge/vendor/tiberius
```

Verify the expected revision from `apps/bridge/THIRD_PARTY_NOTICES.md`:

```bash
git -C apps/bridge/vendor/tiberius rev-parse HEAD
```

If local fork work is lost but it was pushed, recover it from
`origin/xuntuo/sqlserver-2005`. If it was never committed or pushed, Xuntuo
cannot recover it because the parent repository intentionally ignores this
directory.

## Practical Policy

When in doubt:

- official upstream structure wins;
- SQL Server 2005 compatibility invariants remain explicit;
- the patch layer stays small and reviewable;
- the published integration branch is never rewritten;
- every consumed fork revision is pinned and verified;
- driver changes are committed in Tiberius, never in Xuntuo;
- synchronization knowledge belongs in this file rather than in chat history.
