# Flow Wait Until implementation plan

> Execute with superpowers:subagent-driven-development; user authorized direct implementation and a final commit on current main.

**Goal:** Extend existing Flow orchestration with structured inputs, typed statuses, observable Wait Until, and isolated UI run context.

**Architecture:** Preserve FlowService / FlowExecutor / CommandBus. Add canonical `waitUntil`, retain legacy `poll` execution and output semantics. Deserialize old input names as required JSON inputs. Keep JSON storage and historical snapshots, with additive optional run progress fields. No new dependencies.

**Tech stack:** Rust / Serde / Tokio / SQLx; React / TypeScript / shared i18n.

- [x] Core contracts: structured input definitions (`name`, `type`, `required`, optional `default`, `secret`, `description`); strong run/step enums with existing wire values. Reject duplicate names, bad defaults/types, secret defaults. Test serialization and legacy decoding.
- [x] Engine: normalize defaults before execution, enforce required/type checks, union declared secrets before any persistence. Add Wait Until success/failure predicates, `in` membership, fixed interval, optional attempt bound, failImmediately/retryTransientErrors policy. Fresh probe errors classify by typed AppError; failure predicate precedes success. Preserve legacy Poll behavior.
- [x] Observability: persist per-step startedAt, nextCheckAt, final output and attempts; run inspector derives elapsed time, shows last result/error and upcoming check. Scrub all new persisted output surfaces.
- [x] CommandBus: retain typed HTTP status errors for classification, reuse network/timeout errors. Verify probe resource rules and existing execution chain using disposable HTTP/DB fixtures.
- [x] Frontend: reset on Flow selection/new/discard/workspace; synchronize JsonField with parent updates. Add definition editor and typed run form; Wait Until editor with check interval/timeout and advanced attempts; early resource validity; confirmation with masked inputs and context. Keep shared UI design and locale keys.
- [x] Regression checks: frontend tests, core/engine/command-bus Flow tests, typecheck/build, lint changed TS, Rust fmt, migration and large-file checks. Test timeout, cancellation, exhaustion, failure precedence, transient/permanent failures, immutable history, secret persistence and original start-once behavior.
- [x] Review spec coverage and code quality, document compatibility/results in architecture/testing docs, commit task files with Conventional Commit message.

Verification commands: `pnpm exec vitest run packages/flow`; `pnpm --filter @unfour/desktop exec tsc --noEmit`; `pnpm build`; `cargo test -p unfour-core flow`; `cargo test -p unfour-flow-engine`; `cargo test -p unfour-command-bus flow`; `cargo fmt -p unfour-core -p unfour-flow-engine -p unfour-command-bus --check`; `pnpm run check:migrations`; `pnpm run check:large-files`; `git diff --check`.
