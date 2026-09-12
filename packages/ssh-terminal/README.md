# ssh-terminal

## Purpose

`@unfour/ssh-terminal` owns the SSH Terminal frontend experience.

## Local constraints

See [AGENTS.md](AGENTS.md) for this package's scope and invariants.

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
- `src/components/TerminalContextMenu.tsx` - clipboard and selection actions.
- `src/components/HostKeyTrustDialog.tsx` - host-key trust and mismatch UI.

## Current Capabilities

- Create, edit, delete, and select SSH connections.
- Start, close, and track terminal sessions.
- Hydrate session history and append live terminal events.
- Split terminal panes, search, use clipboard/selection context actions, copy
  logs, and export redacted logs.
- Prompt for host-key trust and show mismatch errors.
- Lazily open an SFTP channel from a terminal tab, browse remote files, create,
  rename, and delete entries, and stream one upload or download per SSH
  connection with progress and cancellation.
- Create and run serial SSH tasks composed of command, upload, and download
  steps, with persisted run summaries and logs.

## Known Gaps

- Release readiness belongs in `docs/release/*` and `docs/testing/*`.

## Test / Verify

Choose checks for the changed behavior using the
[verification guide](../../docs/agents/EXECUTION_PROTOCOL.md#choose-verification-by-impact).
The commands below are examples, not a checklist for every edit.

- `pnpm exec vitest run packages/ssh-terminal/src/model/terminal-state.test.ts packages/ssh-terminal/src/model/errors.test.ts packages/ssh-terminal/src/model/sftp-state.test.ts packages/ssh-terminal/src/components/SftpWorkspace.test.tsx`
- `pnpm run build`
- For connection/transport changes, verify affected auth, PTY, trust, reconnect,
  or SFTP paths against an authorized test server. Isolate remote mutations and
  transfers to disposable test data; record unavailable live coverage as not
  verified. UI-only changes do not require the entire SSH regression matrix.
