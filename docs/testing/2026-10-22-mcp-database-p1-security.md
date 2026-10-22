# MCP Database P1 security regressions — 2026-10-22

Base: clean local `main`, `7a3d641f` (`docs(release): prepare v0.12.0 release documentation`).
Scope: outer UPDATE/DELETE WHERE confirmation and one-workspace-per-call binding.

## Root causes and changes

P1-1: MCP's `contains_where()` split the raw SQL text into words. A comment,
literal, quoted identifier, or nested SELECT could supply `where` even when
the outer UPDATE/DELETE had no predicate. The adapter then supplied
`confirm_mutation = Some(true)` to the engine without a guarded confirmation.

The database engine now exposes the pure
`sql_has_outer_write_where_for_driver()` proof through CommandBus. It reuses
the existing `DatabaseDialect`, script boundary checks, and sqlparser dialect
factory; the existing sqlparser dependency parses the statement AST. Only a
single direct UPDATE/DELETE with a parsed `selection` proves an outer WHERE.
Parse errors, unknown syntax, CTEs, EXPLAIN and multiple statements cannot
waive confirmation. Unknown driver names use the existing generic grammar.
When no saved driver is available, every supported dialect must prove the WHERE.
SQL is never rewritten.
MCP replaces the raw-text WHERE search with this proof. Existing risk codes,
confirmation token calculation and permission tiers are retained.

P1-2: SQL driver inference, policy evaluation and Database handlers separately
resolved `CurrentWorkspace`. Switching from A to B between these reads could
apply A's permissions to a B connection. Connection ownership checks did not
help when the handler also supplied B as the execution workspace.

Policy now resolves the target before driver inference and passes that ID to
the lookup. All Database handlers use `ToolPolicyEvaluation.workspace`;
the shared helper retains argument-type validation and performs no workspace
read. Driver lookup also checks workspace membership and live connection
state. No engine ownership, SQL safety, saved read-only flag or native
read-only enforcement was removed or relaxed.

## Failure evidence before the production fix

The following tests were added and run against the original production code:

| Command (`cargo test -p unfour-mcp --lib <filter> -- --nocapture`) | Result and observed failure |
| --- | --- |
| `sql_risk_requires_a_real_outer_where` | FAIL: `DELETE FROM users /* WHERE id = 1 */` incorrectly needed no confirmation. |
| `guarded_forged_where_never_writes_sqlite_before_confirmation` | FAIL: the unconfirmed commented DELETE changed the real disposable SQLite row count from **1 to 0**. |
| `database_workspace_switch` (initial two tests) | FAIL: list operations changed their target; SQL authorization evaluated the later workspace instead of the initial target. |
| `database_workspace_switch_after_write_policy_snapshot` | FAIL: after A/guarded's policy snapshot and a switch to B/read_only, B's real SQLite value changed from **`kept` to `unauthorized`**, returning `affectedRows = 1`. |

Fixtures use an ephemeral CommandBus store and uniquely named temporary SQLite
files. The switching adapter changes the real active workspace immediately
after a selected workspace read; it uses no sleeps or probabilistic race.

## Regression coverage

- Raw line/block/nested comments, ordinary and PostgreSQL dollar strings,
  quoted identifiers, assignment and RETURNING subqueries, CTEs including
  nested writes, malformed SQL, multiple statements and unknown drivers.
- SQLite, PostgreSQL, MySQL and generic dialects; normal outer WHERE writes
  keep their ordinary behavior. CTEs stay conservative, including CTEs whose
  outer mutation has a WHERE, preserving the previous wrapper policy.
- Real SQLite data/row-count/schema/journal checks before confirmation; the
  exact returned token subsequently executes the intended mutation.
- Full access still permits high-risk writes, guarded requires confirmation,
  and read-only blocks writes. Normal conditional guarded writes still work.
- Explicit and omitted workspace IDs, A/B permission combinations,
  same-workspace and foreign connection IDs, policy denial before dispatch,
  and engine rejection of a foreign connection before SQL execution.
- Driver lookup and execution scopes are recorded. An omitted ID reads
  `CurrentWorkspace` exactly once; an explicit ID never reads it.
- Database tool audit: list/create/test connections; list tables/describe;
  query/execute/explain; export confirmation; update/delete connections; and
  history. All use the same policy snapshot, including confirmation payloads.
  Unbounded export tokens match the corresponding explicit-A call after a
  switch to B; deletion is retried with the unchanged confirmation protocol.
- Read-only SELECT calls through execute/query/explain retain
  `enforce_read_only = true` and no mutation confirmation after a switch.

## Final verification

| Check | Final result |
| --- | --- |
| `cargo test -p unfour-mcp -p unfour-database-engine -p unfour-command-bus` | PASS: **595** tests; no failures or ignored tests. CommandBus: 165 unit + 60 integration; Database Engine: 102 unit; MCP: 261 library + 4 binary unit + 3 protocol/registry integration. Windows' process-lifecycle target and doc-test targets contain zero runnable tests. |
| `cargo test -p unfour-mcp policy --lib` | PASS: **30** permission/SQL-policy regressions, including Auto/explicit tiers, disabled/read-only denial, guarded/full-access confirmation and content-bound retry behavior. |
| `cargo check --workspace` | PASS: all workspace crates, including Tauri composition and the desktop adapter. |
| `cargo fmt -p unfour-database-engine -p unfour-command-bus -p unfour-mcp --check` | PASS. |
| `git diff --check` | PASS. |
| `node scripts/check-large-files.mjs` | PASS: zero blocking files; existing large-file advisories retained; no structural refactor or exclusions added. |
| `node scripts/audit-public-secrets.mjs` | PASS. |

Complete local output is in `target/mcp-database-p1-tests.log`,
`target/mcp-database-p1-policy.log`, and
`target/mcp-database-p1-workspace-check.log` (ignored build evidence, not committed).
The runs retain existing release-channel-default and incremental-cache
hard-link fallback warnings; no source warnings were introduced.

The first sandboxed MCP full run could not bind the existing cancellation
test's `127.0.0.1` socket (Windows error 10013). The complete suite was rerun
outside the sandbox for its isolated loopback fixtures. During test authoring,
foreign-connection assertions were corrected to the existing engine contract
`NOT_FOUND`, and confirmation assertions were changed to compare public tokens
because the wire response intentionally exposes only the payload fingerprint.
Production error/confirmation contracts were not changed.

## Limits and remaining risks

- A parsed WHERE proves its syntactic location, not selectivity. `WHERE 1=1`
  can still affect every row under the existing permission semantics.
- Full access intentionally retains its trusted-agent confirmation bypass.
  Guarded unknown/unsupported syntax may prompt more conservatively.
- PostgreSQL/MySQL dialect and existing native-constraint fixture tests are
  covered; no production database or separately deployed real server was used.
- Workspace selection and permissions are a per-call snapshot, not a
  transaction locking concurrent permission edits. A subsequent tool call
  resolves policy again, including confirmation retries.

No Cloud Sync, Flow, persisted schema, release configuration, dependencies,
MCP tool names, JSON Schemas or confirmation/error contracts were changed.
