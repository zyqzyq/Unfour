# SSH tunnel verification

The native integration test runs an SSH server and its targets on loopback. Only the SSH server knows how to reach the test service names.

| Check | Observed result |
| --- | --- |
| SQL targets | Protocol fixtures complete PostgreSQL, openGauss and MySQL queries over routed sockets. The PostgreSQL and MySQL TLS handshakes advertise the original target name. A second physical connection in the same pool also uses the selected route. |
| SQL over SSH | A loopback SSH server carries PostgreSQL SSL negotiation for a saved jump and an already-connected bastion. Both reject an origin that declines required TLS. |
| Temporary API and Redis | The same live bastion forwards an HTTP request and a Redis key check. Redis uses the supplied target password. Both operations reject the closed parent. |
| SSH terminal through a bastion | Two loopback SSH servers perform separate handshakes. The target requires its own host-key check and login, and receives keyboard input. Closing the parent ends forwarding after a single bastion login. |
| Changes during a SQL operation | Changing the saved route in a second window invalidates an outstanding script confirmation and table edit. When the route changes during a draft's SSH handshake, the target receives zero SQL login bytes. |
| SSH route lifetime | Reopening a child terminal from a second window uses its original parent. The SSH task fixture observes execution on the forwarded target. A stalled channel-open request leaves other destinations usable until the bounded pending-request limit is reached. |
| HTTP redirect | A saved environment routes an HTTP request through native SSH to two different HTTP origins. Both receive the original Host and the final response is returned. |
| Completed HTTP writes | A tunneled POST returns 201 while the local history database is busy. The application preserves the successful response when recording history takes longer than the HTTP timeout. |
| Flow and forwarding errors | A Flow runs a saved GET in that environment and the origin receives its original internal hostname. A target rejected by the jump host produces `SSH_TUNNEL_TARGET_UNREACHABLE`. |
| Redis interoperability | A real Redis 8.10.3 process accepts a tunneled connection, authentication and SELECT 2. A confirmed write creates a binary String with a positive TTL and the workbench reads back the same bytes. |
| Host trust and authentication | The SSH driver rejects missing and mismatched pins. The native fixture also exercises an encrypted OpenSSH key, authenticated local proxy access, and closing accepted relays when a lease ends. |
| TLS and failure paths | Transport fixtures inspect ClientHello SNI for the original API and Redis names. HTTP proxy tests cover remote DNS, proxy authentication on multiple hops and disconnection without direct fallback. |
| Desktop interactions | Chromium exercises environment inheritance and a request override through the API toolbar. Its browser transport reports SSH as unavailable. Redis component tests cover selecting a saved jump host and displaying a missing target. |
| Concurrent edits | Route inserts, changes and removals advance the owning workspace's local refresh clock on commit. Rolling back a route edit leaves that clock unchanged. |
| API pre-request scripts | A stale revision for the same jump host stops the request before its script can persist an environment variable. |
| Redis timeout and activity | A stalled SSH handshake exhausts the Redis operation's eight-second budget. A confirmed write rejected during SSH setup records a failed activity. A loopback SSH relay carries AUTH, SELECT and a Redis key check on the forwarded stream. |
| Cancellation and trust changes | A loopback SSH server observes the TCP connection closing when a pending key exchange is cancelled. Removing a trusted host key during the handshake stops authentication. A stale jump-host revision is rejected before dialing. |
| Redis route changes and backup | A new or edited Redis connection refuses a stale jump-host revision during testing and saving. Restoring an encrypted workspace backup preserves a pending route review alongside the restored password. |

Run the transport tests from the repository root.

```sh
cargo test --locked -p unfour-command-bus --features ssh-native --test ssh_tunnel_native
cargo test --locked -p unfour-command-bus --features ssh-native --lib temporary_ssh_commands
cargo test --locked -p unfour-database-engine --lib tunnels
```

For the Redis case, set `UNFOUR_TEST_REDIS_SERVER` to a local redis-server executable and run the same command with `-- --ignored`. The fixture starts a temporary Redis instance with persistence disabled and closes it after the test. The tests use local services. Customer SSH servers and end-to-end deployments with customer-issued TLS certificates require separate verification.
