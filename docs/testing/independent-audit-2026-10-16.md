# Independent global audit — 2026-10-16

## Baseline and scope

- Base: `main`, `5eaa224ff9e54df6555a785a7165127a43ebd723`. `git fetch origin main` succeeded and `main...origin/main` was `0 0` before review. The initial working tree was clean.
- Findings came from source tracing and isolated reproductions in this audit, not earlier audit conclusions. Architecture and ownership documents supplied context only.
- Reviewed the shared `adapter -> CommandBus -> service -> driver` paths and failure boundaries below. This is a risk-focused cross-module audit, not a claim that every line or live deployment was verified.

| Area | Source paths and boundaries reviewed |
| --- | --- |
| SSH | `ssh-engine/src/ssh/native_transport.rs`, `diagnostics.rs`, `task/{mod,runner,upload_step,download_step}.rs`; host-key handler wiring, credential resolution, task cancellation, transfer finalization, MCP command classification |
| Database | `database-engine/src/database/{queries,script_connection,sql,pools,query_history}.rs`, CommandBus database commands; workspace lookup, statement classification, physical connection disposal, result limits and post-execution failures |
| API | `http-engine/src/api_client/{execution,execution_snapshot,history_redaction}.rs`, runtime secret collection, CommandBus execution and script paths; wire versus persistence views, cancellation, bounded response reads |
| Workspace | variable persistence/resolution, delete cascade, CommandBus transactions; workspace ownership, rollback, credential staging and cleanup |
| Flow | engine preparation, runner, storage, expression redaction and CommandBus capability executor; resource snapshots, revisions, cancellation, history and secret provenance |
| MCP | tool policy, SSH operations, SQL validation and Flow serialization; policy classification, adapter reuse, confirmation and returned data |
| Cloud Sync | outbox hook, ownership resolver, push and pull application; local business/outbox transaction, durable owner, account generation fence, immutable retries and cursor commit |
| Import/export | workspace bundle snapshot, remapping, local fields, safety and encrypted backup; coherent snapshot, credential references, import rollback, local paths and imported permissions |
| Community/Pro | unified desktop/MCP composition, account session store, PKCE, entitlement gate and sync token provider; offline local startup, keychain sessions and network eligibility |
| Frontend/release | feature ownership and backend contracts, browser persistence/HTML sinks, full lint/type/build/frontend tests, Rust workspace and native feature compilation |

## Confirmed findings

### P1 — F-01: encoded credentials persisted in Flow history — fixed

Locations: `crates/flow-engine/src/storage.rs:274`, `crates/unfour-command-bus/src/flow_authoring.rs:395`. The common replacement is now `crates/unfour-core/src/redaction/known_values.rs:5`.

Trigger: a Flow API action sends a known credential such as `opaque+value/with space=42`; the response echoes `opaque%2Bvalue%2Fwith+space%3D42` under an ordinary field or header. Similarly, manually declared secret inputs containing quotes/backslashes can be copied into a JSON string under an ordinary key.

Root cause: Flow collected the original secret values but replaced only literal occurrences. HTTP history already recognized encoded representations; Flow's independent replacement loops did not. Structural key redaction does not identify an ordinary `echo` field containing an encoded credential.

Impact: a reversible representation of the credential was written to SQLite `flow_runs.run_json` and could be read through desktop/MCP history. Both successful results and failure diagnostics were affected.

Reproduction evidence: the new `flow_http_encoded_credentials_never_reach_history` test failed against the unmodified implementation with `encoded credential persisted for HTTP 200`. It uses a loopback HTTP server and reads the actual SQLite history row. After the fix it covers both HTTP 200 and 422. `flow_escaped_secret_inputs_keep_runtime_values_but_scrub_persisted_copies` covers JSON-string copies, attempt inputs/outputs and downstream execution.

Fix: put the existing API raw/JSON/form/URL replacement behavior in the shared core redaction owner and use it for API history, Flow persistence and Flow diagnostics before truncation. Keep longer values first and preserve inserted redaction markers. JSON string boundaries are removed exactly rather than using `trim_matches`.

Compatibility: no stored schema, public JSON shape, Cloud Sync protocol, permission rule, dependency or encryption change. Runtime request bodies and downstream step values remain intact. Existing history is readable and is not rewritten. This prevents new leaks; it does not retroactively remove previously leaked values when the original transient secret provenance is no longer available. Historical cleanup requires a separately reviewed approach and, where exposure is confirmed, credential rotation.

### P1 — F-02: mutating SSH commands pass the read-only gate — not changed

Locations: `crates/unfour-mcp/src/tools/ssh_risk.rs:164`, `crates/unfour-mcp/src/tools/policy.rs:180` and `:312`, `crates/unfour-mcp/src/tools/ssh/operations.rs:18`, `crates/ssh-engine/src/diagnostics.rs:64`.

Trigger: invoke `unfour.ssh.exec` in a `read_only` workspace (including `prod` + `auto`) with a connection whose remote account can perform the operation. Commands such as `ip link set eth0 down`, `mount -o remount,rw /`, `hostname audit-name` and `date -s 2030-01-01` are accepted as reads. `unfour.ssh.run_diagnostic` also reaches the engine validator.

Root cause: both MCP and engine allow these utility names without checking their mutating arguments. The engine does restrict several other CLIs, but the listed utilities fall through the unrestricted branch. MCP labels these commands `Read`, permits them under read-only policy, and selects the diagnostic executor.

Evidence: a standalone harness compiled the actual engine validator source and confirmed acceptance of all four strings. No remote commands were executed. MCP classification and dispatch were traced in source. Remote impact is conditional on the SSH account's privileges, not on additional Unfour confirmation.

Impact: read-only automation can change network, mounts, hostname or clock; network changes may disconnect production services.

Proposed change: centralize the diagnostic classification in the owning SSH capability and explicitly allow read-only argument forms; reject write-capable/unknown forms and test them through both MCP and the engine. Preserve the existing broader execution route for authorized non-read-only actions. CLI abbreviations, global flags and platform-specific variants require negative coverage. Existing stored tasks/connections need no migration, but some previously accepted diagnostic strings would be rejected.

Reason not changed: this changes permission policy and compatibility of accepted diagnostic commands. The user's rule 4 requires reporting the risk/plan before implementation. This finding remains a release blocker pending approval and repair.

### P1 — F-03: a committed effect can be reported as an execution failure — not changed

Locations: `crates/unfour-command-bus/src/database_commands.rs:191` and `:230`; the same post-start activity dependency exists in `crates/unfour-command-bus/src/ssh_commands.rs:447`.

Trigger: the target database accepts a confirmed mutation, but the local activity sink subsequently fails (for example local storage is unavailable). `execute_database_query` propagates the activity error after the target write has committed. Row mutation follows the same pattern. Starting an SSH task also precedes its activity write.

Reproduction: in two disposable SQLite databases, register a target containing `effects(n INTEGER)`, then inject the following failure in the local application database:

```sql
CREATE TRIGGER fail_audit_activity BEFORE INSERT ON activity_events
BEGIN SELECT RAISE(FAIL, 'audit sink unavailable'); END;
```

Call `execute_database_query` with `INSERT INTO effects VALUES (1)` and confirmation. The command returns `Err`, while an independent query of the target returns `count(*) = 1`. The temporary audit integration test passed these two assertions and was removed after collecting evidence; it was not retained as a test endorsing the faulty behavior.

Impact: desktop, MCP or Flow can treat an already committed write as failed; retry can duplicate changes. An SSH task may continue while the caller never receives its run handle. The DB fault was executed in the isolated fixture; the SSH analogue is source-confirmed, not a live SSH reproduction.

Proposed change: preserve the authoritative execution result/run handle and expose an explicit activity-persistence warning to each caller, including MCP and Flow; emit a redacted diagnostic. Use the existing batch-script outcome/warning behavior as a consistency reference. Do not swallow the error, report rollback, or automatically retry the operation. Tests should cover query, row mutation and task-start failure, plus caller rendering/serialization.

Reason not changed: this audit deliberately limited implementation to F-01. F-03 requires coordinated result/warning handling across callers; a local `let _ = record(...)` would hide the failure and would not meet the requested behavior. It remains a release blocker, not a passing or deferred-for-release item.

### P2 — F-04: stale-run recovery test races a live heartbeat — recorded

Location: `crates/unfour-command-bus/src/lib_tests/flow_safety.rs:280`.

The fixture starts a real background wait, sleeps 20 ms, overwrites its heartbeat timestamp and immediately reads it. The still-running task can refresh the timestamp or advance the step between those operations. This failed once during the concurrent broad run and passed unchanged in isolation. A deterministic stopped-worker fixture should replace this timing assumption in a separate test-focused change. No production recovery defect is claimed from that failure, and no assertion/test was removed or weakened.

## Verification

Results are for the final audited working tree, including both new integration regressions and the bounded HTTP test fixture.

| Check | Result |
| --- | --- |
| Pre-fix F-01 regression | FAIL as expected: encoded credential in the SQLite Flow row |
| F-01 HTTP success/failure and escaped-input regressions | PASS |
| Isolated F-03 fault injection | PASS: returned error plus committed target row demonstrated |
| SSH validator harness | PASS: write-capable strings accepted; no remote execution |
| Full frontend Vitest | PASS: 181 files, 1,231 tests |
| TypeScript (`tsc -p apps/desktop/tsconfig.json --noEmit`) | PASS |
| Full ESLint | PASS: 0 errors, 34 existing warnings |
| Frontend production build (Monaco preparation + Vite; TypeScript checked separately) | PASS: 2,659 modules; existing large-chunk warning |
| `cargo check --workspace --all-targets --features ssh-native` | PASS |
| Migration check | PASS: 33 files; no migrations changed |
| Version consistency | PASS: 0.11.0 |
| Public secret audit | PASS |
| Rust formatting and `git diff --check` | PASS |
| File-size gate | PASS: 0 blocking; existing size notices retained without unrelated refactoring |
| Final Diff review | PASS: checked secret substitution, real execution values, legacy JSON compatibility, dependency ownership and test failure bounds |
| Full Rust workspace tests (`CARGO_INCREMENTAL=0 cargo test --workspace -j 2 -- --test-threads=4`) | PASS: 1,151 passed, 0 failed, 1 existing OS-keychain smoke test ignored; includes native loopback SSH tests |

Environment failures were investigated rather than suppressed: pnpm's launcher could not open its local database, so the installed Node entrypoints ran the same frontend tools. Sandboxed HTTP tests failed with `NETWORK_ERROR`; the identical test passed with loopback access. Sandboxed Vitest failed to rename its temporary cache (`EPERM`); the normal-permission full suite passed. A baseline test waiting on a blocked loopback accept was stopped by its exact process ID before recompiling. No product validation or test configuration was weakened.

The first full-workspace Rust test build failed during linking with `no space on device` / LNK1180. Only the verified, regenerable `D:\project\Unfour\target\debug\incremental` cache was removed (139.38 GiB); source, Git data and application data were untouched. The retry uses `CARGO_INCREMENTAL=0`, `cargo test --workspace -j 2 -- --test-threads=4` to avoid rebuilding that cache and limit simultaneous link jobs. The failed attempt remains part of the evidence.

F-04 passed unchanged in the final complete workspace run as well as in isolation. The earlier concurrent-run failure is retained as a test-reliability finding. A second pre-commit `git fetch origin main` again confirmed the same base with no divergence. The implementation and this report are committed directly on `main`; `AGENTS.md` is unchanged.

NOT VERIFIED: external production services, real remote SSH/SFTP behavior, real PostgreSQL/MySQL/openGauss servers, OS keychain interaction, signed installers and cross-platform native launch. Compile/unit/loopback results do not replace those checks.

## Release recommendation

Do not advance this state to release sign-off or a release-candidate regression baseline while F-02 and F-03 remain open. Targeted verification of F-01 can proceed. Repair the remaining P1s, then rerun their caller-level regressions and the relevant live-service/platform release matrix. No P0 was confirmed within the audited scope.
