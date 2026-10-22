# SSH task input validation and recall

Baseline: `main` at `fdb55ce9efeea61c270767ae21c08c478faeeafa`.
Verification date: 2026-10-22 (Asia/Shanghai), Windows.

## Behavior and compatibility

- Run preparation retains the ID/revision of each explicitly selected Workspace
  or Environment variable. Execution checks those expectations and resolves
  values from the same SQLite read transaction snapshot. Missing, disabled,
  deleted, renamed, recreated, rotated, or differently sourced records require
  reloading the dialog. Unrelated variables, the active environment, and values
  shadowed by an explicitly selected source do not invalidate that selection.
- Manual values do not acquire variable dependencies. Existing sensitive-name,
  sensitive-value, and secret-alias redaction remains in place.
- The request-only `inputVersions` field contains no values or secret-derived
  hashes. History stays at context version 1; old history and context-free
  explicit-input adapters (including MCP) remain supported. Older request
  contexts decode, but variable references without expected versions fail
  closed and require preparation with the updated client.
- An untouched dialog can restore previous parameters. After an input,
  connection, environment, or sensitivity edit, restore preserves the entire
  dialog and explains that it must be reopened to restore history. Edits made
  while history loads are also retained. Failed/empty history loads preserve
  the dialog; duplicate restore/submit clicks remain coalesced.
- No migrations, dependencies, Cloud Sync protocol changes, or retries added.

## Regression coverage

- Both variable scopes: value/secret-flag edits, disable/delete, rename,
  recreation, additions, identical-value source switches, unchanged parent
  environment timestamps, explicit-source isolation, and manual overrides.
- Concurrent rotation: a run either uses its confirmed value or rejects the
  changed version. A four-connection WAL test exercises atomic Workspace and
  Environment updates and checks metadata/resolver consistency across commits.
- Native local SSH: stale secret versions create no run/history/remote command;
  two concurrent confirmed runs have separate redacted logs and v1 contexts.
- Frontend: unchanged recall, each edit category, edits during deferred history,
  duplicate submissions/restores, failed/empty history, request metadata without
  variable values, and the localized reload error.

## Security boundaries

The read snapshot is the consistency point. Changes committed after it cannot
replace the captured values and do not cancel an already starting/running SSH
command. Revision checks rely on the existing variable mutation paths advancing
record revisions; arbitrary direct database edits are outside that contract.
Client metadata is a stale-input guard, not a new authorization mechanism.
Legacy context-free adapters continue to execute explicit inputs under their
existing confirmation/capability policy. Secrets remain usable in process memory
and on the authorized remote host; this change adds no plaintext persistence.
Unrecognized manual secrets still need the existing sensitive-input marker;
redaction retains its existing heuristics and known-secret matching rules.

## Verification results

These are local equivalents of the relevant CI commands; no remote workflow
was dispatched and no branch was pushed.

| Check | Result |
| --- | --- |
| `pnpm run test` | PASS: 188 files, 1276 tests |
| SSH/command-client tests | PASS: 53 files, 329 tests; dialog follow-up rerun also passed |
| `pnpm run lint` | PASS with warnings, no errors; the dialog hook has size/complexity warnings |
| `pnpm run build`, desktop `tsc --noEmit` | PASS |
| `cargo test -p unfour-command-bus --features ssh-native` | PASS: 225 tests, including native local SSH |
| `cargo test -p unfour-mcp` | PASS: 260 tests, including binary protocol contracts |
| Flow history-limit timing test, isolated recheck | PASS: 1 test |
| `cargo test --workspace --exclude unfour-mcp -- --test-threads=4` | PASS: 997 passed, 1 existing OS-keychain test ignored (requires platform credential store access) |
| `cargo check --workspace` and desktop `ssh-native` check | PASS |
| Affected Rust `cargo fmt --check`, `git diff --check` | PASS |
| Version, public-secret audit, migrations, large-file and shared-token checks | PASS |
| `pnpm run test:release-env` | PASS: 81 tests |

Initial environment-limited attempts: pnpm could not open its cache database
inside the sandbox; sandboxed Rust network fixtures failed and one hung fixture
was stopped. A concurrent rebuild briefly encountered the Windows executable
lock. The pnpm checks and native command-bus suite above subsequently passed
with the required tool-cache/local-loopback access and after releasing the lock.

The first unrestricted `cargo test --workspace --exclude unfour-mcp` attempt
failed in the unchanged Flow history-limit timing test: expected
`FLOW_HISTORY_LIMIT`, received `FLOW_TIMEOUT`. The same test passed in the
native suite and its isolated recheck. No Flow implementation or test was
changed; the workspace suite was rerun with `--test-threads=4` to reduce
contention with other local verification work.
