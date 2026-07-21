# ssh-terminal

## Purpose

`@unfour/ssh-terminal` owns the SSH Terminal frontend experience.

## Boundaries

- Can own SSH connection UI, terminal session UI, host-key trust UI, split and
  search state, terminal logs, and terminal-local Zustand state.
- Should call backend behavior through `@unfour/command-client`.
- May use `@unfour/workspace-core` for selected SSH connection state as the
  documented transitional boundary.
- Should not own API Client, Database, or app-shell behavior.

## Key Files

- `src/TerminalPage.tsx` - top-level terminal page composition and command
  mutations.
- `src/model/terminal-state.ts` - terminal event store and log redaction.
- `src/model/ssh-connection-state.ts` - connection form defaults/conversion.
- `src/model/terminal-tabs.ts` - session tab derivation.
- `src/model/sftp-state.ts` - per-session Remote Files state and persisted panel width.
- `src/components/TerminalWorkspace.tsx` - terminal workspace composition.
- `src/components/SftpWorkspace.tsx` - lazy Remote Files panel, edge handle, and resize boundary.
- `src/components/TerminalPane.tsx` - xterm terminal pane.
- `src/components/HostKeyTrustDialog.tsx` - host-key trust and mismatch UI.

## Current Capabilities

- Create, edit, delete, and select SSH connections.
- Start, close, and track terminal sessions.
- Hydrate session history and append live terminal events.
- Split terminal panes, search, copy logs, and export redacted logs.
- Prompt for host-key trust and show mismatch errors.
- Lazily open an SFTP channel from a terminal tab, browse remote files, create,
  rename, and delete entries, and stream one upload or download per SSH
  connection with progress and cancellation.

## Known Gaps

- Release readiness belongs in `docs/release/*` and `docs/testing/*`.
- Real SSH behavior requires manual verification against a reachable SSH server.

## Test / Verify

- `pnpm test -- packages/ssh-terminal/src/model/terminal-state.test.ts packages/ssh-terminal/src/model/errors.test.ts packages/ssh-terminal/src/model/sftp-state.test.ts packages/ssh-terminal/src/components/SftpWorkspace.test.tsx`
- `pnpm run build`
- For behavior changes, manually verify password/key auth, PTY input/output,
  resize, search, log export, host-key trust, reconnect, SFTP Home discovery,
  remote mutations, transfer cancellation, overwrite behavior, and panel
  isolation against a live SSH server.
