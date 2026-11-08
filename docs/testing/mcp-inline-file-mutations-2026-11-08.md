# MCP inline SSH file mutations — 2026-11-08

## Scope

`unfour.ssh.write_file` now uses a typed command-bus/SSH-engine operation and
SFTP, without a terminal session or local content staging. `patch_file` uses a
fixed Python 3 helper with UTF-8 JSON over SSH stdin; no path/content arguments
are interpreted through the old `python -c ... -- ...` argv offsets.

Inline write content and combined patch search/replace are capped at 4 MiB of
UTF-8 bytes. Patch target/result files are capped at 16 MiB. Ordinary SSH exec
limits are unchanged. The existing `sha2` version is now a direct MCP dependency
to bind file confirmations to content digests without disclosing the content.

## Verification

| Check | Result | Coverage |
| --- | --- | --- |
| `cargo test -p unfour-mcp --lib tools::ssh::tests` | PASS, 33 tests | Exact/over 4 MiB, UTF-8 byte accounting, all write modes, combined patch limit, structured limit details, content-bound confirmations, guarded sensitive multi-match and full-access behavior, production policy and activity warnings |
| `cargo test -p unfour-mcp --lib atomic_replace_unsupported_keeps_safe_code_and_actionable_message` | PASS, 1 test | Stable unsupported-atomic-replace code and safe, actionable MCP message |
| `cargo test -p unfour-ssh-engine --features ssh-native --lib file_operations` | PASS, 6 tests | Actual Python 3 execution, multi-MB Unicode, quoted/non-ASCII paths, CRLF, ASCII stdin locale, incomplete input, invalid UTF-8, no/multiple matches, deletion, target and expansion limits |
| `cargo test -p unfour-ssh-engine --features ssh-native --lib inline_sftp` | PASS, 12 tests | In-memory SFTP protocol: 4 MiB chunked writes; partial write, metadata, fsync, close and commit failures; failed cleanup; cancellation/deadline before/after atomic commit; lost OPEN/commit replies; runtime shutdown; unsupported extension; append prefix preservation |
| `cargo test -p unfour-command-bus --features ssh-native ssh_file_patch_native --no-fail-fast` | PASS, 1 test | Loopback SSH: complete 4 MiB stdin, payload absent from exec command, EOF-before-exit-status, workspace isolation, timeout, metadata-only audit, activity failure without replay |
| `cargo check -p unfour-mcp --no-default-features` | PASS | Non-native feature combination |
| `cargo fmt -p unfour-core -p unfour-ssh-engine -p unfour-command-bus -p unfour-mcp --check` | PASS | Touched Rust crates |
| `git diff --check` | PASS | Whitespace |
| `node scripts/check-large-files.mjs` | PASS | 0 blocking; existing size advisories remain |

During iteration, a visibility compile failure and an incorrect test assumption
that workspace IDs must be UUIDs were fixed. Python subprocess tests initially
failed under the sandbox temporary directory; isolated fixtures now live under
the repository's ignored `target/ssh-file-tests` directory and pass.
`pnpm run check:large-files` failed with sandbox `EPERM` creating a pnpm symlink;
the identical script was then run directly with Node and passed. Cargo also
reported existing no-native dead-code, default release-channel, and incremental
cache hard-link fallback warnings.

## Limits of this evidence

No user host or production workspace was modified. Real deployed OpenSSH/SFTP
server verification is NOT VERIFIED; SFTP tests use a protocol server in memory,
and the SSH transport test uses a disposable loopback server. Python tests use
the local Python interpreter, not a remote host's installed Python version.
The running desktop/MCP binary was not replaced or restarted.

Overwrite requires the server's `posix-rename@openssh.com` version 1 extension
when the target already exists. Unsupported servers fail before mutation; no
backup-move fallback exists. Patch uses a sibling temporary file and replacement with
metadata conflict detection. Extended ACLs/attributes and hard-link identity
are not preserved. Append can partially complete; timeout/cancellation and
disconnects require inspecting remote state before retrying. Cleanup after a
disconnect is best effort. See [tool semantics](../mcp/tools.md) for details.

## Follow-up safety review

The initial implementation reused the upload helper's two-step backup/rename
sequence. Cancellation or disconnection after moving the original could leave
the destination absent, and recovery depended on asynchronous cleanup racing
with disconnect. The inline operation now uses one raw SFTP channel (same
authentication, configuration and library), because the library's high-level
session does not expose POSIX rename. All data/metadata, optional fsync, and
close responses must succeed before the one atomic replacement request.
No code path renames or removes the destination as a cleanup action.

Cleanup only owns an acknowledged file handle and a uniquely named sibling.
An unacknowledged OPEN can leave an empty staging file. An unacknowledged commit
can leave the complete new target, so a timeout is not evidence of rollback.
Neither case deletes the original before a completed replacement. No automatic
retry is performed, and append keeps the original prefix even if only part of
the new content was accepted. These are network/cancellation guarantees, not
server power-loss or concurrent-external-writer guarantees.

Fault injection also exposed a recursive Drop-cleanup task during runtime
shutdown in an intermediate revision (FAIL: stack overflow). The final cleanup
task captures plain resources rather than another Drop guard. The dedicated
shutdown regression and all 12 SFTP cases now PASS.
