# Redis workbench verification

These checks exercise the development source on Windows. The desktop UI and native Redis driver were verified separately.

| Check | Evidence |
| --- | --- |
| Standalone Redis interoperability | A loopback-only Redis 8.10.3 Windows MSYS2 process with persistence disabled was launched for this test and stopped afterward. The native Rust driver read different values for the same key in DB 0 and DB 2, distinguished persistent and expiring keys, matched a Hash field and handled a missing key. A restricted ACL user could read its keys and received a permission error for SCAN. |
| Protocol edge cases | Loopback RESP fixtures exercised byte-preserving keys, bounded String/Hash reads, scan limits, expiry during a read, timeouts and rejection of plaintext on a TLS connection. |
| Database UI | Chromium exercised keyboard-driven Redis connection creation, an exact key lookup, switching between SQL and Redis tabs, and opening a new Redis check in Flow. Redis network results in this browser test are fixtures. Component tests cover stale responses, Hash field retries and display limits. |
| Workspace lifecycle | A migration test upgraded a populated legacy schema. Local tests covered checked connection edits, credential replacement and removal, portable export, encrypted backup and restoration, and isolation across workspaces. |
| Flow and MCP | An exact-key Wait Until ran through the command bus against a loopback protocol fixture. Regression tests checked results and the absence of private operands in history. MCP tests exercised the registered read-only tool and its workspace policy. |
| Cloud Sync | Client-side tests use a mock transport to exercise connection metadata, deletion intent and Redis actions in Flow definitions. The worker also retries the original operation after an uncertain push and a local password rotation. Real hosted-service acceptance and two-device sync require separate validation. |

## Collection browsing and desktop writes

- The loopback Redis 8.10.3 acceptance test browses Hash, List, Set and ZSet, checks a binary String update with its key TTL, rejects an outdated value, sets and removes expiry, and deletes a single key. Hash checks cover creating the first field, duplicate-add rejection, changing and deleting one field while keeping its sibling, and preserving the field's absolute expiration time on update.
- Protocol fixtures exercise stale values, an absent key at edit time, an aborted WATCH transaction, loss of the EXEC reply and the different Hash field expiry capabilities of Redis 6.x, 7.4 and 8.x. Only 8.10.3 was exercised with a real server.
- Collection component tests cover filters, empty scan batches with a continuation, forward/back navigation, list positions, sorted set ordering and stale results. Connection and workspace tests cover the default after migration, rejecting writes with an old revision or missing confirmation, and the local grant after remote edits, ordinary import and encrypted backup restoration.
- Range regressions exercise a 50-member page followed by a member over the byte limit, and a final page with exactly 50 members. Confirmation tests distinguish a literal key named `HEX ff` from the binary byte `ff`. A key containing a newline keeps its original bytes when the workbench reads it back after a write.
- Write-dialog tests simulate a changed or deleted connection and a failed first refresh of the connection list. Retrying the refresh only reloads saved connection definitions. The client does not replay the write or inspect the key on a different endpoint.
- Chromium opens a Redis connection from Database, enables local writes, previews and applies a String creation with a 120-second TTL, reads it back, confirms a single-key deletion and reads the missing-key result. The browser uses a simulated command transport. Component tests distinguish a failed read after a successful write from a failed write and prevent a second submit after an unknown outcome.
- A command-bus test drops the connection after the fixture receives EXEC and checks the resulting SQLite activity record. Its target is the connection ID and its details contain only the operation and outcome. Cloud worker tests check that changing the grant creates no additional push and preserves the payload of an uncertain retry.

Review regressions cover empty-string assertions, stale connection-list responses, SQL selection while a Redis tab is open, and type changes during a Hash field lookup. Flow history tests check the JSON types of the recorded conclusion when a private operand happens to equal a metadata value such as `true`. MCP save and get tests validate a Redis Flow against the advertised JSON schemas.

To repeat the standalone check, set `UNFOUR_TEST_REDIS_SERVER` and `UNFOUR_TEST_REDIS_CLI` to matching local executables, then run:

```sh
cargo test --locked -p unfour-redis-engine --test standalone -- --ignored
```

The test allocates a local port and creates its own temporary authentication material. The Windows artifact used here was the non-service MSYS2 zip with SHA-256 `AF16DF340525B431E9EC8DABFD625D5E7A233F82805B5418819904EA909C447F`. [Versioned fixture release](https://github.com/redis-windows/redis-windows/releases/tag/8.10.3)

Native packaged UI, a TLS deployment with a trusted certificate, Sentinel and Cluster were outside these checks. Runtime and portable-format details are in the [Redis design](../architecture/redis-workbench.md).
