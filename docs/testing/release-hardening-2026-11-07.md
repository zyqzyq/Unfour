# v0.13.0 release hardening — 2026-11-07

Status: **local automated verification complete; NOT RELEASED / RC NOT STARTED**.
This is a source governance record, not a release or real-service sign-off.
The earlier [preparation record](v0.13.0-release-preparation-2026-11-07.md) remains
historical evidence for its own source. No push, tag, workflow dispatch,
deployment or external-service mutation is part of this round.

## Source and execution

- Previous release: `v0.12.0`, `4883df8dbfaf4d9d69c94ed5bae75c627d146e34`.
- Entry: clean `main`, `48df06828de6a642ca4b195bf56692fd0835e8d8`.
  Fetch updated `main` and the local/remote-tracking heads agreed. Fetching all
  tags reported an unrelated conflicting `v0.1.0`; that tag was not overwritten.
  The entry is 12 commits after v0.12.0. No new branch was created.
- Final implementation: `545894f61939b6dcf52d273fc003d90d9ffa9e3a`.
  Frontend/build/browser checks ran at `0a2e65b4`; the subsequent implementation
  commit changes only Rust SSH history scoping and its tests. Final Rust checks
  run at `545894f6`. Documentation is committed after verification.
- Windows x64, Node 24.16.0 / Cargo 1.96.0. Final Rust commands set
  `UNFOUR_RELEASE_CHANNEL=stable`, `UNFOUR_DISTRIBUTION=standard`, and
  `UNFOUR_BUILD_COMMIT` to `git rev-parse HEAD`. Earlier targeted Rust batches
  used the default Test channel; they are not signed-release build evidence.
- Logs are local, ignored artifacts under `target/hardening-v0.13.0/`.
  Results, failures and limitations are preserved here; logs are not committed.

## Audit and ranked plan

The initial read-only pass inventoried the repository, release delta, manifests,
local instructions, size scan, lint warnings, public facades and critical call
chains. Review then followed failure paths and existing regression fixtures.
This is broad repository coverage with focused source inspection, not a claim
that every line or possible concurrent schedule has been proved correct.

| Area reviewed | Existing boundary / finding | Decision |
| --- | --- | --- |
| Database engine | `database.rs` is already a facade; drivers, pools, script connections and transactions have owners. `sql.rs` mixes construction and policy. Script connections use close-on-drop; read-only setup fails closed. | Extract SQL safety without changing its implementation. Retain pool/cancel/transaction semantics and verify SQL/driver fixture tests. |
| SSH engine | Terminal, diagnostics and Task repeat connect configuration, host-key handler and timeout. Session close/cancel signal transfers before checking workspace ownership. | Share the dial boundary; separate auth/supervision; fix teardown and history-flush scope. |
| HTTP engine / API Client | Execution snapshots, sensitive history, auth, domain storage and cURL parsing already have owners; redaction delegates to core. | Retain current contracts, verify send/save/history/script/cURL regressions; do not invent another secret sanitizer. |
| CommandBus | Domain transactions invoke hooks before commit; command guards, credentials and activity handling have dedicated modules. | Preserve adapter -> bus -> service chain. Do not duplicate policy or move transactions into adapters. |
| MCP | Shared policy, confirmation, output and adapter error modules already exist. Database tool registration and handlers are interleaved. | Extract registration/schema only; preserve exact definitions, registration order and adapter trait. |
| Frontend Database / API / SSH | React Query keys carry workspace/context; existing controllers guard stale saves and query refresh. Terminal history rejection can evict a newer marker. | Fix the confirmed history race; keep existing query/error/i18n infrastructure. |
| UI / shell | Reusable feedback/loading/error primitives and feature mounts are already separated. | No new design system, feature imports into UI, or controller moved to app-shell. |
| Account | Generation-scoped refresh, callback/replay handling and retry diagnostics already exist. | Retain implementation; run frontend account/deep-link and Rust account tests. Live OAuth remains separate acceptance. |
| Cloud Sync / local refresh | `refreshLocalQueries` is shared by local commit and sync notifications; repository owner filters and transactional outbox capture remain authoritative. | No new polling/cache mechanism, no change to owner/outbox contracts; verify existing worker, isolation and refresh tests. |
| Dead code / compatibility / dependencies | Long forwarding and compatibility paths are not proof of dead code. | No speculative deletion, dependency upgrade or lint suppression. Remaining candidates are ranked in the size audit. |

Execution order was low-risk responsibility extraction, focused reliability fixes,
then broad regression and documentation. Bugs were reproduced before fixing.

## Actual changes and responsibility changes

| Files (repository relative) | Change |
| --- | --- |
| `crates/database-engine/src/database.rs`, `database/sql.rs`, new `database/sql_safety.rs` | Public SQL safety exports keep their names; safety parsing/classification is independent of identifier/browse/export SQL construction. |
| `crates/unfour-mcp/src/tools/database.rs`, new `database_registry.rs` | Database registration, descriptions and schemas move together; handlers, validation and confirmation stay with the existing module. |
| `crates/ssh-engine/src/ssh.rs`, `ssh/native_transport.rs`, new `ssh/native_auth.rs`, new `ssh/native_supervisor.rs` | Facade wiring plus explicit dial/auth/supervisor responsibilities. No unused routing abstraction. |
| `crates/ssh-engine/src/ssh/diagnostic_execution.rs`, `task/native.rs` | Terminal, diagnostics and Tasks use one native dial implementation; existing caller-specific error wording is retained. |
| `crates/ssh-engine/src/ssh/session.rs` | Share workspace-aware native detachment; guard history flush before persistence. |
| `crates/ssh-engine/src/ssh_tests/mod.rs`, new `ssh_tests/native_lifecycle.rs` | Three regression cases for ownership-before-side-effect and legitimate owner behavior. |
| `packages/ssh-terminal/src/hooks/useActiveSessionHistory.ts` and `.test.tsx` | Ignore obsolete rejection cleanup; add three lifecycle regressions. |
| Architecture/testing/release Markdown | Replace the stale size inventory, document transport ownership and acceptance evidence, link the new checkpoint from current release records. |

There are 16 implementation/test files changed. No package manifest, lockfile,
database migration, persisted model, import/export schema, Cloud Sync payload,
Tauri command declaration, MCP public trait or permission configuration changes.
No product feature or user-visible string is added. `v0.13.0` is unchanged.

| File | Entry lines | Final lines |
| --- | ---: | ---: |
| Database `database/sql.rs` | 856 | 388 |
| MCP `tools/database.rs` | 1005 | 598 |
| SSH `ssh/native_transport.rs` | 598 | 159 |
| SSH `ssh/diagnostic_execution.rs` | 178 | 143 |
| SSH `task/native.rs` | 169 | 157 |
| SSH `ssh/session.rs` | 789 | 779 |

The new files hold coherent responsibilities, not arbitrary line chunks:
SQL safety, MCP registration, SSH authentication and SSH supervision.
The full scan moves from **95 to 93** files over 500 lines, **24 to 22 P0**, with
**0 Critical / 0 blocking / 0 exceptions** throughout. Existing thresholds and
exclusions are unchanged. See the [complete inventory](../architecture/large-file-audit.md).

## Reliability regressions

1. **Foreign-workspace close/cancel:** previously returned a scoped lookup error
   only after signalling SFTP transfers. New tests reproduce both paths, assert
   neither transfer nor supervisor cancellation and no generation/status change,
   then verify the owner can cancel, persist disconnected state and release the
   session. Detachment releases the session lock before taking the transfer lock.
2. **Foreign-workspace history read:** previously returned an empty history while
   flushing another workspace's pending output. The test demonstrates that write
   side effect, then verifies the fix retains the foreign empty result and the
   owner still flushes and reads the output normally.
3. **Obsolete frontend history rejection:** deactivate/reactivate, complete the
   second read, then reject the first. Previously this erased the newer marker
   and caused a third fetch. Tests also cover retry after a current failure and
   suppression of completions after workspace switch/unmount.

Existing default/native session tests continue to cover history failures,
idempotent close, reconnect bounds, command tracking and cleanup. No tests or
compatibility branches were deleted.

## Commands and actual results

Node entrypoints are the installed commands behind the package scripts; no
dependencies were installed or upgraded. `pnpm` wrappers were not needed for
these direct invocations. Playwright used its unchanged configured pnpm server.

| Executed command | Result |
| --- | --- |
| `node node_modules/eslint/bin/eslint.js .` | PASS; 0 errors, 43 pre-existing warnings, same count as initial scan. Warnings include function/file length, complexity and React effect state updates. |
| `node node_modules/typescript/bin/tsc -p apps/desktop/tsconfig.json` | PASS |
| `node scripts/prepare-monaco.mjs` then `node node_modules/vite/bin/vite.js build apps/desktop` | PASS; production build, existing >500 kB chunk-size warning. |
| `node node_modules/vitest/vitest.mjs run` | PASS; 215 files / 1,573 tests. |
| `CI=true node node_modules/@playwright/test/cli.js test --workers=2` | PASS; 48 Chromium smoke tests. PowerShell invocation used `$env:CI='true'`. Browser transport is mocked. |
| `cargo fmt --all --check` | PASS |
| `cargo check --workspace --offline --locked` | PASS |
| `cargo check -p unfour --features ssh-native --offline --locked` | PASS |
| `cargo test --workspace --offline --locked -j 2 -- --test-threads=4` | PASS; 1,313 tests, 0 failures, 1 intentionally ignored OS-keychain smoke. |
| `cargo test -p unfour-ssh-engine --offline --locked -- --test-threads=4` | PASS; final rerun 105 tests. |
| `cargo test -p unfour-ssh-engine --features ssh-native --offline --locked -- --test-threads=4` | PASS; final rerun 109 tests including the 3 scope regressions. |
| `cargo test -p unfour-database-engine -p unfour-mcp -p unfour-ssh-engine --offline --locked -- --test-threads=4` | Batch 1 PASS; 479 tests including 104 Database, 269 MCP and 106 SSH-native. |
| `node node_modules/vitest/vitest.mjs run packages/ssh-terminal/src/hooks/useActiveSessionHistory.test.tsx packages/ssh-terminal/src/hooks/useTerminalSessionActions.test.tsx packages/ssh-terminal/src/hooks/useTerminalSessions.test.tsx` | Batch 2 PASS; 13 tests. Changed frontend files also pass targeted ESLint. |
| `cargo test -p unfour-ssh-engine --features ssh-native --offline --locked native_lifecycle -- --test-threads=4` | PASS; 3 final scope regressions. |
| `node scripts/sync-version.mjs --check` | PASS; 0.13.0 |
| `node scripts/release-contract.mjs v0.13.0` | PASS; validation only, no tag creation. |
| `node scripts/check-migrations.mjs` | PASS; 37 migrations. |
| `node scripts/audit-public-secrets.mjs` | PASS; final scan 1,591 publishable files. |
| `node scripts/check-shared-tokens.mjs` | PASS; 110 shared tokens. |
| `node scripts/check-large-files.mjs --json --top 200` | PASS; 93 findings, zero blocking; no thresholds relaxed. |
| `git diff --check` and final Markdown link/file checks | PASS; 6 Markdown files, 33 local links and 5 heading fragments; diff whitespace clean. |

Release-script suite executed exactly:

```text
node --test scripts/release-environment.test.mjs scripts/release-channel.test.mjs scripts/release-contract.test.mjs scripts/release-candidate-contract.test.mjs scripts/check-update-order.test.mjs scripts/finalize-standard-release.test.mjs scripts/msix-contract.test.mjs scripts/check-large-files.test.mjs
```

Result: **PASS, 81 tests**. These include MCP-independent release/distribution
contracts; MCP tool/protocol/permissions are covered by the Rust MCP suite and
the preserved registry comparison below.

Source comparison against entry `48df0682` also confirmed exact moved bodies for
the SQL classifier, SSH authentication, and MCP registration plus describe-table
schema. The MCP comparison preserves names, descriptions, input/output schemas,
annotations and registration order. This is additional evidence, not a substitute
for executable contract tests. Unchanged command and persisted contract paths
were checked against the Git diff. A literal-import scan across frontend packages
found zero forbidden cross-feature, feature-to-app-shell/workspace-local, or
UI-to-feature imports. This static check does not resolve computed imports.

### Failures and recovery (not counted as PASS)

- Initial batch compilation: **FAIL**, MCP registration re-export visibility.
  Corrected to the enclosing tools module's visibility; subsequent batch passes.
- Sandbox Database batch: **FAIL**, 19 temporary-export rename / loopback socket
  permission errors, 85 tests passed. Identical batch outside the sandbox passed.
- Sandbox targeted Vitest: **FAIL**, cache rename `EPERM`, no test executed.
  Ordinary-permission rerun executed the regression and reproduced its assertion.
- Regression red runs: **FAIL as expected**, stale frontend request 3 calls
  instead of 2, foreign close/cancel transfer cancellation (2 tests), and foreign
  history flush clearing the owner's pending buffer (1 test). Each passes after
  its fix.
- Initial final workspace Rust tests: **FAIL before suite completion**, Windows
  `LNK1180: insufficient disk space` linking MCP `local_data_consistency`; E had
  about 5 MB free. After confirming no Cargo/rustc/link process was active, removed
  only the resolved workspace `target/debug/incremental` cache (about 89 GiB of
  logical files; about 74 GiB free space recovered). No source, user data or other
  directory was removed. Final rerun limits build parallelism to `-j 2`; test
  coverage and assertions are unchanged. The final workspace and both SSH feature
  suites completed successfully.
- Earlier targeted sandbox compilation reported incremental hard-link fallback
  warnings. These are environment/cache warnings, not new source lint allowances.

## Regression coverage and real-environment limits

| Journey | Automated evidence in this run | Remaining acceptance |
| --- | --- | --- |
| API send/save/history/cURL | HTTP engine, CommandBus, API frontend and cURL smoke | Authorized real HTTP endpoint, native credentials and packaged history: NOT VERIFIED. |
| SSH Terminal/SFTP/Task rerun | Native engine/SFTP fixtures, frontend sessions, task rerun and preview smoke, new scope tests | Real SSH host connect/reconnect, transfers, interruption, rerun and native terminal: NOT VERIFIED. |
| Database SQL/batch/export | SQLite tests, PostgreSQL/MySQL/openGauss protocol fixtures, safety/read-only/transaction/result-cap and frontend tests | Real PostgreSQL/MySQL/openGauss, network cancellation and packaged exports: NOT VERIFIED. |
| Flow execution/failure/history | Engine, bus, MCP and browser Flow suites | Packaged runs using real API/SSH/DB services: NOT VERIFIED. |
| MCP API/SSH/DB/Flow | Tool, policy, confirmation, binary protocol and cross-process temporary-storage tests | Real client against production-policy workspace and remote resources: NOT VERIFIED. |
| Workspace import/export/variables/permissions | Workspace/storage/bundle/policy tests and browser smoke | Upgrade from a disposable v0.12.0 profile and native encrypted backup: NOT VERIFIED. |
| Account/login/Cloud Sync | Account/deep-link/refresh tests, outbox ownership/worker tests, local refresh tests | OAuth/deep-link in installed app; offline/retry/conflict/deletion across real devices: NOT VERIFIED. |
| Platform distribution | Source build/check/release contracts | Windows/macOS/Linux installer, updater, signing and native credential stores: NOT VERIFIED; no RC packages built. |

Mock and protocol-fixture tests never establish native WebView event delivery,
real-server compatibility or user acceptance. The existing ignored OS-keychain
smoke remains a separate manual gate; it must not be reported as executed.

## Remaining risks and next phase

- Remaining oversized files and 43 ESLint warnings are explicit debt, not hidden
  by exceptions. Prioritize concrete ownership changes in MCP forwarding, SSH
  controllers, SFTP transfer lifecycle and Database export when their behavioral
  fixtures justify the change; see the current size audit.
- SSH authentication/channel setup/teardown do not have a single end-to-end
  cancellation budget; Task connect completes before step-level cancellation.
  The encrypted-key path does not pass a passphrase into decryption. These are
  existing limits preserved in this no-feature round, not promises of support.
- Native network teardown, transport reconnect races and platform keychains need
  loopback SSH server fixtures and live acceptance. No full transport proxy or
  connection-pooling abstraction was introduced to anticipate unspecified needs.
- There is no evidence here to authorize publication. Recommend these internal
  refactors and correctness fixes within **v0.13.0**, following a reviewed exact
  candidate SHA, hosted CI, Standard RC and applicable manual gates.

For Bastion, extend the shared dial boundary, define lifetime ownership of parent
hops, keep host-key/credentials per hop and test cancellation/partial failure
before adding persisted routing fields. Full guidance and preserved boundaries
are in [SSH transport ownership](../architecture/ssh-transport-boundary.md).

Local automated source gates are complete. Release readiness remains conditional
on the manual/native gates above and subsequent exact-SHA hosted CI/RC.

## Commits

| Commit | Scope |
| --- | --- |
| `616af783` | `refactor(core): separate SQL policy and share SSH transport setup` |
| `0a2e65b4` | `fix(ssh): scope teardown and preserve current history hydration` |
| `545894f6` | `fix(ssh): scope terminal history flush to its workspace` |
| Commit containing this record's final revision | Governance inventory, transport design constraints, verification and release pointers. |

All commits remain local on `main`. No release artifact, temporary log or build
output is intentionally tracked.
