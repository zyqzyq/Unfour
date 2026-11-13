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

Review regressions cover empty-string assertions, stale connection-list responses, SQL selection while a Redis tab is open, and type changes during a Hash field lookup. Flow history tests check the JSON types of the recorded conclusion when a private operand happens to equal a metadata value such as `true`. MCP save and get tests validate a Redis Flow against the advertised JSON schemas.

To repeat the standalone check, set `UNFOUR_TEST_REDIS_SERVER` and `UNFOUR_TEST_REDIS_CLI` to matching local executables, then run:

```sh
cargo test --locked -p unfour-redis-engine --test standalone -- --ignored
```

The test allocates a local port and creates its own temporary authentication material. The Windows artifact used here was the non-service MSYS2 zip with SHA-256 `AF16DF340525B431E9EC8DABFD625D5E7A233F82805B5418819904EA909C447F`. [Versioned fixture release](https://github.com/redis-windows/redis-windows/releases/tag/8.10.3)

Native packaged UI, a TLS deployment with a trusted certificate, Sentinel and Cluster were outside these checks. Runtime and portable-format details are in the [Redis design](../architecture/redis-workbench.md).
