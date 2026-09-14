# MCP diagnostic completion verification — 2026-09-14

Baseline: `zyqzyq/Unfour` main at
`60db6e46f99023b01a69923c4587f5345d029553`, verified against the remote main
over HTTPS. Tests ran on Windows against ephemeral storage and loopback HTTP
servers; no user database or SSH host was used.

## Changes

- Added `unfour.db.list_history`, `unfour.db.update_connection`,
  `unfour.db.delete_connection`, `unfour.ssh.update_connection`,
  `unfour.ssh.delete_connection`, `unfour.ssh.test_connection`, and
  `unfour.ssh.get_host_key`.
- New adapter methods delegate to existing Command Bus history, save/delete,
  connection-test and fingerprint paths. No engine/domain logic was copied.
- Added a caller-owned execution ID entry point for saved API replay, delegating
  to the existing controlled script execution path. Existing replay callers and
  signatures retain their behavior.
- Stdio keeps protocol controls responsive, accepts scoped cancellation, bounds
  execution and queued calls, and releases its runtime lock before awaiting work.
- No new dependency was added. Tokio's existing `sync` and `time` features are
  now explicit because the MCP adapter directly uses watch channels and timers.
- Updated `docs/mcp/tools.md`, `docs/mcp/overview.md`, and `CHANGELOG.md`.

## Verification

| Check | Result |
| --- | --- |
| `cargo test -p unfour-mcp` (default includes ssh-native) | PASS: 192 library, 4 binary, 2 registry integration tests |
| `cargo test -p unfour-mcp --no-default-features` | PASS: 192 library, 4 binary, 2 registry integration tests |
| `cargo test -p unfour-command-bus` | PASS: library and API/connection/SSH-task/workspace domain integration suites |
| Actual ephemeral binary `initialize` + `tools/list` | PASS: 63 tools; all seven additions have input/output schemas and all four annotations |
| New success `structuredContent` against output schemas and `_meta` | PASS, including SSH failure summaries and known/unknown fingerprint projections |
| Guarded connection deletion | PASS: missing/wrong confirmation rejected; revision change invalidates old confirmation; exact current confirmation deletes |
| Workspace boundaries | PASS: cross-workspace/missing/deleted IDs rejected; prod/auto, explicit read_only and disabled deny mutations |
| History and credential masking | PASS: SQL literal/quoted-token/comment/dollar-quote masking; workspace filtering and limits; omitted credentials preserved; no credentials/key paths/raw SSH error text in results |
| Cancellation | PASS: unlimited HTTP call closes its actual loopback connection; ping/discovery remain responsive; later calls succeed; queued cancellation and tool notifications do not mutate workspace variables |
| Saved API cancellation | PASS: caller execution ID cancels HTTP, cleans the registry and skips incomplete HTTP history |
| Safety deadline | PASS: pending execution terminates under a shortened test deadline; pre-registration cancellation never starts execution |
| `cargo fmt -p unfour-mcp -p unfour-command-bus --check` | PASS |
| `git diff --check` | PASS |
| `node scripts/check-large-files.mjs` | PASS: zero blocking files |

Initial test failures were corrected: old tool-count assertions, a duplicate
workspace name in the new fixture, and a blocking test thread join that prevented
the current-thread Tokio runtime from completing HTTP socket cleanup.
`pnpm run check:large-files` failed in the local pnpm launcher/cache before running
the script; running the exact package script directly with Node passed.

## Limits and non-goals

- Unix signal process tests are NOT RUN on Windows (`cfg(unix)`). The real
  binary's EOF/registry/outbox tests and in-process disconnect tests passed.
- Real SSH authentication and remote database execution/cancellation are NOT
  VERIFIED against live services. Native SSH compiles and its adapter contracts
  pass; remote process/SQL termination remains best effort.
- The 120-second MCP deadline is independent of the HTTP timeout. Cancellation
  cannot undo committed remote writes, committed script changes, or already
  started asynchronous SSH tasks. See the overview for cleanup/queue behavior.
- Connection updates are metadata-only and preserve credentials; credential
  rotation remains in Desktop. Fingerprint reads never change host trust.
- No Flow, HTTP transport, interactive SSH, automatic troubleshooting workflow,
  policy mutation, raw credentials/known_hosts, or the excluded CRUD, file
  binding, batch, import/export and reorder capabilities were added.
