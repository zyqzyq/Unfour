# SFTP read-only text preview — 2026-10-30

Base: local `main` at `1c497c0a` (clean working tree at task start).

## Scope and contract

- Regular-file context menus expose Preview. Existing double-click, download,
  upload, delete, and directory navigation handlers retain their behavior.
- The shared Dialog shows filename, byte size, scrollable plain text, explicit
  Copy content, and Close/Escape. English and Chinese use shared locale keys.
- `previewSftpText` → `ssh_sftp_preview_text` → CommandBus → SSH Engine reuses
  workspace/session authorization and the existing SFTP channel. This command
  is desktop-only; no new MCP surface is introduced.
- The backend limit is fixed at 1,048,576 bytes. LSTAT and opened-handle FSTAT
  reject known oversized/non-regular files. The driver receives bounded read
  buffers; at most 1,048,577 bytes are read, with one sentinel byte detecting
  growth or inaccurate/missing size metadata. No full-file download then trim.
- Strict UTF-8 decoding accepts an optional initial UTF-8 BOM. The size includes
  the BOM; displayed/copied text omits it. NUL and non-text ASCII control bytes
  trigger binary detection; UTF-16 BOM and invalid UTF-8 trigger encoding errors.
  Detection is content-based, not an extension allowlist: `.log`, `.txt`, `.json`,
  `.yaml`, `.yml`, `.xml`, `.conf`, `.ini`, `.sh`, `.sql`, `.env`, and other UTF-8
  regular files can be read. ANSI escape sequences are rejected as control data.
- Errors use fixed local codes, never server error messages or file bytes.
  Oversize, binary, encoding, permission, disconnect, timeout, missing file,
  non-regular file, and other read failures have localized UI messages.
- Reads have a 30-second overall timeout and explicit handle close, with the
  driver's drop cleanup on cancellation/timeout. Closing the UI discards its
  request subscription; it does not cancel an already-issued backend read.
- Content lives only in a mounted preview's state/promise. No React Query cache,
  Zustand store, local file, transfer, activity, history, or persistence receives
  it. Copy writes to the clipboard only on user action. No remote mutation
  protocol requests are issued (server-managed access timestamps may change).
- Workspace/session/connection changes remount preview state. Disconnect clears
  text. Closed or superseded requests cannot update the new view; a final backend
  check also rejects inactive/replaced SFTP generations. Strict Mode effect replay
  shares the mounted preview's request.
- No dependencies added; no editor/highlighter, editing, saving, tailing, or diff.

## Automated verification

| Check | Result |
| --- | --- |
| Preview Dialog, FileList, Panel, Workspace, locale Vitest suites | PASS; 43 tests across 5 files after adding Workspace/session return regressions |
| Command-client native invoke contract | PASS; 1 test, success invokes only the scoped preview command |
| `cargo test -p unfour-ssh-engine --features ssh-native sftp --lib` | PASS; 17 tests |
| `cargo test -p unfour-ssh-engine sftp --lib` | PASS; 9 tests |
| `cargo check -p unfour --features ssh-native` | PASS; desktop/Tauri/CommandBus/native SSH integration compiles |
| `pnpm --filter @unfour/desktop exec tsc --noEmit` | PASS |
| `pnpm run build` | PASS; existing >500 kB bundle warning remains |
| ESLint on changed TypeScript/TSX | PASS, zero errors; existing oversized SftpPanel function warning remains |
| `cargo fmt` check on core, SSH Engine, CommandBus, app | PASS |
| `pnpm run check:large-files` | PASS; zero blocking files, existing size advisories remain |
| `git diff --check` | PASS |
| `pnpm exec playwright test sftp-text-preview.spec.ts` | PASS; 2 browser fixture tests, English/light and Chinese/dark, static PTY placeholder |

The Rust tests use an in-memory SFTP protocol server and disposable SQLite
memory pool with no schema. They verify READ-only open flags, close on success
and failure, bounded actual wire reads despite false metadata, UTF-8/BOM, binary
and encoding rejection, permission errors at open/read, transport disconnect,
wrong Workspace, non-regular files, changed session generation, and no transfer
or terminal events. This is not real Linux/SSH authentication verification.

The frontend tests cover demand-only reads, all requested extensions, plain-text
rendering, explicit copy and clipboard failure, all error messages, close/reopen,
late success/error, disconnect, Workspace/session/connection switches, return to
an earlier session, Strict Mode, and absence of mutation/transfer/local-dialog
calls or cached/persisted content. Browser screenshots were visually inspected
for both themes; overflow and Escape are also asserted.

## Failed attempts and limits

- Full-page browser run with the real xterm renderer: **FAIL**. Preview behavior
  assertions passed, but the page emitted `Cannot read properties of undefined
  (reading 'dimensions')` from xterm's deferred `_sync` / refresh callbacks.
  This unrelated renderer was not modified. The final scoped smoke fixture uses
  a static TerminalPane; its PASS does not establish full-page PTY compatibility.
- Initial Strict Mode duplicate preview read was fixed and regression-tested.
  Initial test fixture/assertion issues were corrected before the passing runs.
- One Rust rebuild failed with `no space on device` on D:. Two verified old SSH
  Engine incremental-cache directories under `target/debug/incremental` were
  removed, and the final native test run passed after rebuilding.
- **NOT VERIFIED:** real Linux/SFTP manual validation, explicitly deferred by
  the user. Actual server permissions, disconnect/reconnect, large files, remote
  filesystem behavior, and native desktop clipboard still need manual validation.
