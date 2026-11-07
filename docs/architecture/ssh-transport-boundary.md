# SSH transport and session ownership

Reviewed on 2026-11-07 for v0.13.0 hardening. Direct SSH remains the only
implemented route. This document describes current ownership and the next
Bastion design constraints; it does not introduce routing configuration.

## Current call chain

Tauri / MCP -> CommandBus -> SshService -> native transport -> channel consumer.
Policy, confirmation and workspace environment gates remain in their existing
CommandBus / MCP paths. The transport module does not replace those checks.

| Owner | Responsibility |
| --- | --- |
| `ssh/native_transport.rs` | Shared `dial_native_connection`: client configuration, 15-second connection timeout, workspace/host/port host-key handler and sanitized connection failures. `open_authenticated_connection` composes dial/auth for terminal and diagnostics. Terminal PTY/shell creation remains a channel consumer here. |
| `ssh/native_auth.rs` | Existing password, private-key and none authentication. Transient overrides and SecretStore references retain their existing semantics. |
| `ssh/native_supervisor.rs` | Terminal reader, output batching, reconnect backoff, cancellation, health events and generation-aware handle installation. |
| `ssh/session.rs` | Public session operations, workspace-scoped teardown, history persistence and in-memory registry lifetime. |
| `ssh/sftp.rs` and children | SFTP channels cached per terminal session/generation, path validation, preview/transfer operations and transfer cancellation. |
| `task/native.rs` and step drivers | A fresh task transport using the same dial/auth capability; serial exec/upload/download steps, cached task SFTP and task-owned disconnect. |
| `ssh/diagnostic_execution.rs` | Fresh authenticated transport, validated one-shot exec, bounded output and diagnostics timeout. |

Terminal/SFTP share a session transport. A Task or diagnostic uses its own
transport; sharing the **connection capability** does not mean pooling all these
consumers onto one physical connection. There is no new public trait or proxy
configuration, and no additional dependency.

## Teardown and workspace isolation

`detach_native_session` checks live-session ownership before setting intentional
close, signalling the supervisor or invalidating SFTP. It takes the handle while
holding the session mutex, releases that lock, then cancels transfers. Network
close/disconnect and history persistence occur outside the mutex. This preserves
lock ordering and prevents a caller in another workspace from cancelling a
transfer before the session lookup rejects it.

Failed-connect cleanup uses the same detachment and keeps its best-effort
cleanup behavior. Close/cancel still remove the in-memory entry even if final
history persistence fails. Repeated close uses the existing persisted-summary
fallback. History hydration flushes live output only for the owning workspace;
the persisted read keeps its prior workspace-scoped empty-result behavior.

Reconnection rejects a newly opened handle when the session was intentionally
closed in the meantime. Installing a replacement invalidates cached SFTP state
and cancels transfers from the previous connection. This round preserves the
existing generation and reconnect policy rather than adding a new lifecycle.

## Next Bastion / Jump Host phase

1. Extend the internal dial boundary first. Keep endpoint authentication and
   channel consumers separate from how the endpoint byte stream is obtained.
   Do not add route fields to today's records until persistence, export and sync
   compatibility have an explicit design.
2. Define ownership of the entire hop chain. A successful connection must retain
   every parent hop required by the final stream; failure, cancellation and
   reconnect must close child channels and their parent handles in reverse order.
3. Keep host-key identities and credential references per hop, scoped to the
   workspace. A proxy address must never substitute for the destination identity.
   Do not persist resolved secrets or log private keys, tokens or passwords.
4. Establish cancellation and time budgets for dial, each authentication phase,
   channel setup and teardown. Today the shared 15-second budget covers connect,
   not the entire authenticated session setup. Task cancellation is observed by
   step execution after driver connection; making connection setup cancellable
   needs dedicated lifecycle tests.
5. Preserve reconnect generation semantics for Terminal/SFTP and isolated Task
   run ownership. Prefer a concrete direct-plus-one-hop implementation before a
   general multi-hop abstraction; introduce a transport owner only when needed.
6. Add local SSH server fixtures for host-key mismatch at each hop, partial
   authentication, cancellation during handshake, dropped parent transports,
   concurrent terminal/SFTP use, reconnect and Task cleanup. Then test authorized
   real hosts and packaged native clients.

## Known limits

The existing encrypted-private-key path reads a credential reference but does
not pass a passphrase into a decryption operation. This round moves that code
unchanged; encrypted-key support is not newly claimed. Authentication and
teardown cancellation/timeouts, along with this compatibility question, must be
resolved explicitly before promising Bastion support. Agent forwarding,
interactive MFA, arbitrary ProxyCommand and multi-hop configuration are outside
this round.

Automated SFTP fixtures and workspace-state tests do not establish live SSH
interoperability. See [hardening verification](../testing/release-hardening-2026-11-07.md)
for executed results and remaining manual acceptance.
