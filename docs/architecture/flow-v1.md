# Flow V1

Flow is a local Workspace runbook module alongside API, SSH, and Database.
It composes existing capabilities; it is not a general workflow platform.

## Ownership and call path

`packages/flow` owns the vertical step editor and run inspector. It imports
shared command-client and UI contracts, never another feature package.
App-shell provides navigation, lazy mounting, and a sidebar slot only.

`command-client -> Tauri adapter -> CommandBus -> FlowService -> FlowExecutor`
is the execution path. `crates/flow-engine` owns validation, expressions,
serial scheduling, cancellation, and SQLx persistence. Its executor port has
`prepare` and `execute` methods. CommandBus implements that port using the
existing API client, SSH task service, and Database query command. No HTTP,
SSH protocol, credential storage, or database driver was added to Flow.
The HTTP engine exposes saved-auth materialization for callers without a UI.

## Persisted model

- `flow_definitions`: stable UUID, workspace ID, revision, definition JSON,
  updated timestamp. Saves use an optimistic revision check.
- `flow_runs`: stable UUID, workspace ID, originating Flow ID, status,
  cancellation flag, run JSON, start timestamp, heartbeat timestamp.
- Run JSON contains its own definition revision, explicit invocation context,
  prepared resource snapshots, node records, and attempt records. Attempts
  contain resolved input, structured output, stable error code, and duration.

Deleting a definition leaves its runs and all referenced resources intact.
Deleting a workspace cascades its Flow records. Every lookup is workspace
scoped. Editing a definition cannot change an existing Run.

Flow V1 is local-only and is not part of Cloud Sync or workspace import/export.
It does not emit cloud outbox intent because these records are not cloud-bound.

## Invocation and expressions

`FlowRunInput` explicitly provides `workspaceId`, `flowId`, `environmentId`,
`inputs`, `initiator` (`human` or `mcp`), and `confirmEffects`. Optional
`secretInputNames` identifies sensitive top-level inputs with custom names.
An absent/null environment means workspace variables only, never the UI's
active environment. Required input names are declared in the definition.
Environment variables in a saved API resource resolve when the Run prepares.

Inputs are lightweight definitions: `name`, `type` (`string`, `number`,
`boolean`, `json`), `required`, optional `default`, `secret`, and optional
`description`. Names must be nonblank and unique. Runtime values are checked
without coercion; absent values receive defaults before required/type validation.
JSON accepts any JSON value, including null. Explicit null defaults are retained;
an omitted default remains absent. Extra runtime keys remain accepted for V1
compatibility. Secret defaults are rejected, including defaults under known
sensitive names. Declared secret names and per-run secret names both feed
redaction, including validation-failed runs.

Use `{"$ref":"/inputs/version"}` to preserve a JSON value's type, or
`"job=${/steps/start/body/jobId}"` to interpolate text. Paths use JSON Pointer
semantics, including `~0` and `~1` escaping. Missing paths fail the node.
Condition predicates have `left`, `op`, and `right`; operators are `eq`, `ne`,
`gt`, `ge`, `lt`, `le`, and `in`. Ordered comparisons require numbers; `in`
tests whether the resolved right-hand array contains the left-hand JSON value.
There is no expression evaluator or Script node.

Text interpolation is literal, including SQL and SSH inputs; it does not
escape or parameterize them. Only trusted values should enter command or SQL
text. Existing SSH task substitution and Database SQL safety still apply.

## Nodes and execution

The ordered step list runs serially. An omitted `next` proceeds to the next
step, a forward step ID jumps ahead, and `$end` completes successfully.
Condition selects exactly one of `ifTrue` / `ifFalse`; skipped nodes are
recorded. Branch arms should explicitly jump to their common continuation or
`$end` to avoid falling through into the other arm. Backward jumps are rejected.
The first error stops subsequent scheduling. There are no implicit retries.

| Node | Reference / arguments | Output |
| --- | --- | --- |
| API Request | Saved request ID; optional `url`, `headers`, `query`, `body` overrides | `status`, `headers`, JSON-or-text `body`, `durationMs`, `historyId` |
| SSH Task | Task ID, explicit connection ID, `inputs` object | `runId`, `status`, timestamps, redacted `log`, `logTruncated` |
| Database Query | Connection ID; `sql`, optional `catalog`, `schema`, `limit` | Existing Database query result, including columns and rows |
| Condition | Predicate and forward destinations | `matched` |
| Wait Until | Probe, success/failure predicates, fixed interval, total timeout, optional attempt limit and probe error policy | `result`, `attempts`, `elapsedMs` |
| Legacy Poll | Existing probe/predicate/interval/attempt limit | Original matching probe output, preserving old reference paths |
| Wait | Duration shorter than node timeout | `waitedMs` |

All referenced resources, including unchosen branches, are checked before any
node executes. Missing references produce a persisted `validationFailed` Run.
Resource identity is checked again before execution; changes fail explicitly.
API execution uses its prepared request. SSH and Database retain their owning
services' resource lookup semantics, with a small check-to-execution race if a
resource is edited concurrently. Resource locks/versioned execution are future
work, not a claim of transactional isolation across services.

HTTP status >= 400 produces a typed HTTP status error. Actions and legacy Poll
fail immediately; Wait Until applies its probe error policy. V1 does not retain the failed response body
in Flow output. Saved API scripts and multipart requests are rejected rather
than silently ignored. SSH logs are capped at 64 KiB; they are not a separate
typed stdout/stderr/exit-code tree.

## Wait Until

New definitions use `kind: "waitUntil"`. Each check executes a fresh probe;
on a successful response it evaluates `failureWhen` first, then `successWhen`.
A matching failure condition ends with `FLOW_FAILURE_CONDITION`, even if both
conditions match. A successful condition returns:

```json
{"result":{"status":200,"body":{"state":"success"}},"attempts":8,"elapsedMs":35210}
```

Later nodes use `{"$ref":"/steps/waitDeployment/result/body/state"}`. The
existing JSON Pointer reference syntax remains unchanged.

`probeErrorPolicy` defaults to `failImmediately`. `retryTransientErrors`
retries typed network/timeouts, HTTP 408/429/5xx, and database pool timeouts or
connection I/O failures. Permanent errors still fail fast. No diagnostic-message
parsing is used. Local history/persistence failures are never treated as probe
failures or retried. Each attempt retains its safe error code, including HTTP
status; response bodies of failed HTTP probes are not retained by Flow.

`intervalStrategy` is currently `fixed`; `intervalMs` is the delay after a
completed unsuccessful check. `timeoutMs` bounds the whole node, including
probes and waits. Optional `maxAttempts` counts all checks, including failed
probes, and defaults to no extra attempt cap. An explicit cap must be 1–1,000.
Timeout and existing history-size guards still apply when no cap is supplied.
The UI emphasizes Check every and Timeout, with the cap under Advanced.

Step history stores `startedAt`, `nextCheckAt`, attempts, and final `output`.
The inspector derives live elapsed time from the step start and shows the
latest result/error and next check. Finalization clears `nextCheckAt`.

```json
{
  "id":"waitDeployment", "name":"Wait for deployment", "kind":"waitUntil",
  "timeoutMs":60000, "intervalStrategy":"fixed", "intervalMs":1000,
  "maxAttempts":null, "probeErrorPolicy":"retryTransientErrors",
  "probe":{"capability":"api","resourceId":"saved-status-request-id","arguments":{}},
  "successWhen":{"left":{"$ref":"/probe/body/state"},"op":"eq","right":"success"},
  "failureWhen":{"left":{"$ref":"/probe/body/state"},"op":"in","right":["failed","cancelled"]}
}
```

The scope remains external state checks, not general Action Retry.

## Legacy Poll compatibility

Each attempt performs `probe -> condition -> wait -> probe`. `/probe` is
replaced with the newly returned output before testing the predicate. A Poll
never reschedules an earlier start action. For example, POST a job once in an
API step, then probe its status using the job ID:

```json
{
  "id": "poll", "name": "Wait for job", "kind": "poll",
  "timeoutMs": 30000, "intervalMs": 1000, "maxAttempts": 20,
  "probe": {
    "capability": "api", "resourceId": "saved-status-request-id",
    "arguments": {"url": "https://service/status/${/steps/start/body/jobId}"}
  },
  "predicate": {"left": {"$ref": "/probe/body/ready"}, "op": "eq", "right": true}
}
```

API probes must use GET/HEAD. Database probes require a read-only connection
and the existing SQL safety checks. SSH probes are not supported in V1.
Method/connection restrictions express a read intent; the server remains
responsible for GET semantics. Probe failures fail fast. Exhaustion records
`FLOW_MAX_ATTEMPTS`; the timeout covers all probes and waits together.

Definitions allow 1–100 steps; node timeouts are 1–3,600,000 ms, check intervals
10–60,000 ms, and attempt limits 1–1,000. Inputs are limited to 256 KiB,
prepared snapshots to 2 MiB, each output to 256 KiB, and accumulated node
history to 4 MiB. Database row limits are capped at 1,000. These are execution
guards, not a database retention policy.

## Cancellation, recovery, and sensitive values

Cancellation persists intent, stops subsequent scheduling, and signals the
active executor. HTTP uses the existing cancellation token, SSH uses task
cancellation, and Database drops the query future. A bounded two-second grace
allows the active action to observe cancellation/timeout. Remote effects may
already have occurred and are never described as rolled back.

Running nodes heartbeat every 100 ms. Reads mark a run with a heartbeat older
than 30 seconds as `interrupted`; unfinished steps become interrupted/skipped.
There is no automatic replay after process loss, avoiding duplicate effects.

Execution values live in memory; persisted and returned Run views are redacted.
Known sensitive keys/headers and explicitly named secret inputs are removed,
including copies in outputs. Inline secrets in recognized sensitive definition
fields are rejected; references to runtime inputs are allowed. Existing
credential references remain references. Historical definitions are redacted
snapshots, not credential backups or automatically replayable payloads.
Arbitrary secrets embedded under innocuous names cannot be inferred reliably;
use `secretInputNames` and existing credential storage. Underlying capabilities
retain their own history/redaction policies.

## Data compatibility and UI context

No SQLite schema migration is needed: definitions and runs retain JSON storage.
Reading legacy `inputs: ["value"]` normalizes each name to a required JSON
input, preserving historically accepted number/boolean/object values. Saving a
definition writes the structured form under the existing revision check.
Historical Run JSON is not rewritten when a definition is edited or read.
Old runs without progress/output fields decode with null defaults. Existing
`poll` nodes keep their original execution/output contract; no reference-path
rewrite or automatic node conversion occurs. Both node variants are labeled
Wait Until in the UI, while new nodes use the richer contract above.

Run and Step statuses are enums, preserving all existing serialized values:
run `running`, `succeeded`, `failed`, `timedOut`, `cancelled`, `interrupted`,
`validationFailed`; step adds `pending`/`skipped` and excludes `validationFailed`.
The frontend uses the corresponding string unions.

Flow selection, New Flow, accepted discard, and workspace changes reset run
inputs to that definition's defaults, reset manual secret names, and clear the
environment selection to workspace-only. Environment is deliberately scoped
to the selected Flow invocation, not an implicit workspace-level preference.
JsonField synchronizes external values while retaining invalid local edits
until reset/correction. Input definitions drive typed run fields. Run confirmation
shows the Flow/workspace/environment and masks declared/manual/known-sensitive
inputs. The editor preflights listed resource metadata for missing resources,
API scripts/multipart, GET/HEAD probes and read-only DB connections; Rust still
enforces the authoritative execution checks.

## Future callers and current limits

CommandBus already exposes `list_flows`, `get_flow`, `save_flow`, `delete_flow`,
`run_flow`, `list_flow_runs`, `get_flow_run`, and `cancel_flow_run`. A future MCP
adapter can invoke these same methods and provide `initiator: mcp`; this change
does not register any Flow MCP tools. Initiator is provenance, not permission.

The UI polls the selected active Run and, while History is open, summaries
containing running Runs. `flow_runs_list` / `listFlowRuns` return at most 100
`FlowRunSummary` records (id, flowId, status, startedAt, finishedAt), read from
independent columns without loading or decoding `run_json`. History loads on
open; selecting an entry loads its full snapshot with `getFlowRun`. Completion
invalidates History for its next open or active refresh. The summary migration
adds `finished_at`, backfilled once from snapshots; writes and stale-run recovery
keep it equal to snapshot finishedAt. Snapshot semantics are unchanged. There is
no pagination, pruning, or deleted-Flow history
browser. Retained runs remain accessible by ID through the service. Definition
drafts survive module switching but are not persisted across workspace changes
or application restarts. Browser preview supports authoring only; execution
requires the desktop backend and is never simulated as success.

Parallelism, Sub-flow, arbitrary loops, Script, Scheduler, Webhook, LLM/Agent,
Plugin, Team/RBAC, automatic resume, and Flow Cloud Sync are outside V1.
