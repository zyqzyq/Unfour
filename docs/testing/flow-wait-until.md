# Flow Wait Until verification

Date: 2026-09-20. Extends [Flow V1 verification](flow-v1.md); the previous
record remains historical evidence. Contract: [Flow architecture](../architecture/flow-v1.md).

## Results

| Check | Result |
| --- | --- |
| `cargo test -p unfour-core -p unfour-flow-engine -p unfour-command-bus --features unfour-command-bus/ssh-native` | PASS: 133 tests (29 core, 2 engine, 102 command-bus unit/integration) |
| `cargo check --workspace` | PASS |
| `cargo fmt -p unfour-core -p unfour-flow-engine -p unfour-command-bus --check` | PASS |
| Vitest: Flow, command-client, UI, app-shell, workspace-core | PASS: 181 tests across 43 files |
| Final focused Flow Vitest run | PASS: 23 tests across 5 files |
| Desktop TypeScript check and Vite production build | PASS; existing large-chunk build warning remains |
| ESLint: Flow sources and command-client Flow types | PASS: zero errors; three warnings for component length, deliberate JsonField effect synchronization, and resource-validation complexity |
| Migration checker | PASS: 28 migrations; no migration added |
| Large-file checker | PASS: zero blocking files |
| `git diff --check` | PASS |

Direct local Node entry points were used for Vitest, TypeScript, ESLint and
Vite. No dependency or lockfile change is needed.

## Behavioral coverage

- Legacy input names decode as required JSON inputs. Structured definitions
  reject empty/duplicate names, invalid typed defaults, and secret defaults.
  Runtime validation applies defaults without coercion, including false, zero
  and explicit JSON null. Declared secrets are registered before validation.
- Run/step enums retain wire values. Old runs without progress fields decode;
  reading/editing definitions does not rewrite persisted historical Run JSON.
- Wait Until executes fresh probes, checks failure before success, supports
  `in`, counts errored attempts, and returns the latest result with attempts
  and elapsedMs. A preceding start action executes exactly once.
- Typed transient errors retry only under the selected policy. Loopback HTTP
  fixtures cover 503/429/408 followed by success and permanent 401 failure.
  Local persistence and size-limit errors cannot enter the probe retry loop.
- Timeout, cancellation, attempt exhaustion, next-check progress and final
  output limits are covered. Prior API/Condition/native SSH/SQLite tests pass.
- Declared/manual/sensitive-key values are scrubbed from persisted snapshots,
  attempt data and final output, including validation-failed malformed inputs.
- API probe method, API scripts/multipart, missing resources and read-only DB
  constraints are validated before execution; frontend metadata preflight
  marks these problems before Run.
- React regressions cover Flow/new/workspace/discard context reset; external
  JsonField synchronization; incomplete independent JSON edits; typed fields
  and masked confirmation; schema validation; save-in-flight selection guards;
  stable input-row drafts after deletion; schema-type changes clearing obsolete
  field validity; and invalid defaults requiring discard before navigation.
- The run inspector retains the last completed result and last error while a
  new attempt is in flight, identifies the error attempt, and updates elapsed
  time without waiting for backend progress.

## UI inspection and limits

PASS: local browser preview with the real React shell and Chinese locale.
Inspected New Flow, structured default values populating the run form,
Wait Until interval/timeout/success/failure/error policy controls, early missing
resource errors, secret fields, confirmation context with masked values, and
New Flow clearing environment/manual secrets/inputs. Only disposable browser
authoring data was saved; no external service was invoked.

The narrow in-app preview (about 606 px wide) compresses the existing two-column
editor and can require horizontal scrolling. This remains a desktop layout
limitation; responsive editor layout is useful follow-up work.

NOT VERIFIED: packaged Tauri interactive execution against production services,
PostgreSQL/MySQL live connectivity, or remote shell termination guarantees.
Backend evidence uses disposable loopback HTTP/native SSH fixtures and SQLite.
Browser preview remains authoring-only and cannot establish desktop execution.

## Issues found and resolved

The implementation checks exposed and fixed final-output history accounting,
redaction of malformed failed-run inputs, persistence errors incorrectly
eligible for probe retries, and a heartbeat future retaining a single-pool
connection after work completed. A Windows HTTP test fixture now restores
accepted sockets to blocking reads and rejects incomplete headers.

Frontend regression failures exposed missing input-name validation, switching
during Save, losing last probe details during an in-flight check, and indexed
input keys discarding another row's invalid JSON. Each has a passing regression.
