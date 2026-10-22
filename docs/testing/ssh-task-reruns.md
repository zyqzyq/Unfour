# SSH task reruns verification

Verified on Windows on 2026-10-22, based on `main` / `081b6982`.

## Behavior and persistence

- Ordinary Run uses current workspace and active environment defaults. Previous
  parameters are restored only through the explicit dialog action or a completed
  history/result-panel rerun action. Every entry opens the same editable dialog.
- Failed, successful, and cancelled runs can be prepared again. Execution uses
  the current saved task definition and the existing serial runner, with a new
  run ID and log file. Original records and logs are retained under the existing
  retention policy. This does not resume a failed step or automatically retry.
- Missing connections, deleted environments, unresolved source variables, and
  missing required inputs block execution. Definition/environment changes show
  a review notice; changes after preparation reject stale execution metadata.
- Preparation generations isolate task/workspace changes and closed dialogs.
  In-flight starts are coalesced, and stale completions cannot attach to a new
  workspace. The existing cancellation, transfer, and log lifecycle is reused.
- Migration `20261021000000_core_ssh_task_run_context.sql` adds one nullable
  `ssh_task_run.context_json` column. There is no new table or backfill. NULL,
  malformed, and unknown-version context is treated as legacy context and uses
  current defaults. The upgrade test preserves existing status and log paths.

## Security boundary

The versioned local context contains the selected environment ID/version, task
definition version, input sources, secret markers, and permitted manual values.
Workspace/environment values are never copied into the snapshot. CommandBus
resolves their sources again at execution, checks the selected environment, and
rejects inconsistent source reads instead of silently using a workspace fallback.

Rust filters explicit secrets, shared sensitive-name patterns, recognizable
credential/header/connection-string values, known workspace/environment secret
aliases, and the selected connection credential from manual recall. Known secret
values also feed the existing task log/event redaction. A failed credential lookup
disables manual value recall conservatively. The dialog supports marking an
otherwise innocuous input name as sensitive, and preserves secret provenance when
restoring history. No context enters Cloud Sync domain snapshots or workspace
bundles; native tests assert both exclusions. No entitlement or new dependency
was added, so the UI and shared execution path apply to Community and Pro.

An arbitrary opaque string under an innocuous manual field cannot be recognized
as a credential from its content alone: mark such inputs sensitive or use secret
workspace/environment variables. The existing redactor does not promise detection
of arbitrary server-side transformations of secrets. Local safe manual values are
plain JSON, not an encrypted vault.

## Checks

| Check | Result |
| --- | --- |
| SSH frontend Vitest suite | PASS: 43 files, 254 tests; final affected model/dialog/panel tests rerun after refinements |
| `cargo test -p unfour-ssh-engine --features ssh-native --lib task` | PASS: 38 tests |
| `cargo test -p unfour-command-bus --features ssh-native --lib ssh` | PASS: 16 tests |
| `cargo test -p unfour-command-bus --lib ssh_context` | PASS: 3 tests |
| `cargo test -p unfour-local-storage --lib ssh_task` | PASS: 3 tests, including upgrade from the previous schema |
| `cargo test -p unfour-mcp --lib ssh` | PASS: 35 tests |
| `cargo test -p unfour-ssh-engine --lib task` | PASS: 34 tests without native SSH |
| Playwright `ssh-task-rerun.spec.ts` | PASS: ordinary defaults, explicit recall, deleted connection, secret re-entry, history and panel entry, no direct execution |
| Desktop TypeScript + Vite production build | PASS; existing large-chunk warning |
| `cargo check -p unfour --features ssh-native` | PASS |
| Affected crate `cargo fmt --check`, frontend ESLint | PASS; broader SSH lint reports five existing size warnings in unchanged components, zero errors |
| Migration validator, large-file gate, `git diff --check` | PASS; 36 migrations, zero blocking large files |
| Public repository secret audit | PASS after changing a synthetic PEM test marker to runtime construction; no real key material was present |

The native rerun test uses an isolated loopback SSH server and disposable SQLite
and log data. It verifies full execution after a failed middle step, current task
definitions, skipped disabled steps, cancellation followed by a fresh run,
independent histories/logs, and export/sync exclusions. Negative tests cover
authorization, cookie, proxy authorization, API keys, auth tokens, PEM material,
credential URLs, explicit secrets and aliases, rotated environment values,
foreign/deleted environments, deleted connections, legacy records, repeated
clicks, and task/workspace/close/unmount races.

Initial browser attempts failed because test locators did not match existing
accessible control names; those locators were corrected and the test passed.
Initial sandbox attempts could not create the default Vitest cache or bind native
test sockets. Vitest passed with a workspace-local temporary cache; native and
browser tests passed with approved local execution after a temporary approval
service failure. Browser screenshot was visually inspected at 1280 x 720.

NOT VERIFIED: packaged Tauri GUI against a real external SSH host or production
credentials. The browser test uses UI fixtures; native loopback tests independently
exercise the real CommandBus, service, SSH driver, storage, cancellation, and logs.
