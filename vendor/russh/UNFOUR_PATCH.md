# russh 0.61.1: verify host keys during rekey

This is the complete published `russh` 0.61.1 crate, including its normalized
`Cargo.toml`, original `Cargo.toml.orig`, examples, tests, and source. The Cargo
registry cache marker `.cargo-ok` is omitted. No dependency version or feature
selection is changed. The root workspace applies it with `[patch.crates-io]`.

## Source and licensing

- Registry package: `russh` 0.61.1.
- Original crate SHA-256 (Cargo.lock):
  `f67013f080c226e5a34db1c71f2567f44d95a6300005bb6cd4e2c8fe3c326d1b`.
- Official repository: <https://github.com/warp-tech/russh>.
- Published `.cargo_vcs_info.json`: commit
  `a2ffd0fafd2522793761b12ebc9496007e4b287a`, directory `russh`.
- License: Apache-2.0. The published crate omits the repository-level license;
  `LICENSE.txt` is copied unchanged from that exact upstream commit:
  <https://github.com/warp-tech/russh/blob/a2ffd0fafd2522793761b12ebc9496007e4b287a/LICENSE.txt>.
  License file SHA-256:
  `cfc7749b96f63bd31c3c42b5c471bf756814053e847c10f3eb003417bc523d30`.
- `UNFOUR_BASELINE_SHA256.json` lists the original byte-level SHA-256 of every
  retained registry file. `unfour-rekey.patch` is the complete source patch.

## Why this local patch is necessary

Upstream 0.61.1 calls `check_server_key` only for initial key exchange, after
`kex_done`. Although its exchange implementation checks each exchange's server
signature, that verifies possession of the newly presented key, not whether it
matches the application's trusted host. `kex_done` cannot inspect the host key.
Unfour therefore previously disconnected temporary connections on every rekey.
The default rekey limit is one hour (or the byte limit), evaluated on transport
activity; enabling 60-second keepalive would expose this lifetime restriction.

The **only modified upstream file is `src/client/mod.rs`**: the existing
`check_server_key` invocation is moved to the common `KexProgress::Done` path,
before `kex_done` and before installing new keys, and its documentation now
describes the per-exchange call. A rejected key still returns `UnknownKey`.
This preserves all cryptographic signature verification, negotiated algorithms,
and the original time/byte rekey limits. No synthetic terminal input or repeated
authentication is introduced. New checks also apply to ordinary pinned clients.

Unfour's temporary trust callback separately consumes fresh validation evidence
on each exchange, matches the initial transport pin as well as current durable
and workspace pins, and rechecks lifecycle/cancellation after async lookup.
Missing verification, a changed key, or a closed workspace remains fail closed.

## Review and validation

Compare all copied files against `UNFOUR_BASELINE_SHA256.json`: only
`src/client/mod.rs` should differ. Apply `unfour-rekey.patch` to the published
baseline to reconstruct that file. Extra local files are this note, the baseline
manifest, the patch, and the upstream `LICENSE.txt`.

Targeted application tests live in:

- `crates/ssh-engine/src/ssh/temporary/tests/trust_tests/rekey_tests.rs`:
  fresh verification per exchange, changed initial key, durable pin conflicts,
  missing callbacks, and cancellation/workspace-close races.
- `crates/ssh-engine/src/ssh/temporary/keepalive/tests/rekey.rs`:
  repeated real SSH rekeys with continuing PTY input and one authentication;
  a changed expected pin is rejected on rekey without authentication replay.

The release/testing records hold actual command results and desktop validation.
Remove this patch when a reviewed upstream release provides equivalent
per-exchange host-key verification and ordering; retain the regression tests.
