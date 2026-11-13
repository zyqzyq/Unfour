# Unfour SQLx transport extension

This directory contains the published sqlx-core 0.8.6 source from crates.io, licensed under MIT or Apache-2.0. Both upstream licenses accompany it.

PoolOptions can take a TCP socket factory. For each physical connection, the pool scopes that factory to the driver's connect future, including connections opened by pool maintenance. The socket layer uses it before local DNS or TCP and rejects Unix socket selection for such a pool. The PostgreSQL and MySQL drivers continue to perform their own handshakes and TLS hostname verification against the configured destination. The extension is gated on the Tokio runtime.

Cargo resolves registry patches at the workspace root. A separate workspace that consumes Unfour through a submodule must point its own `[patch.crates-io]` entry for `sqlx-core` at this directory when updating the submodule.
