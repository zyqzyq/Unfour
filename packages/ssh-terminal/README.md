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
- `src/components/TerminalWorkspace.tsx` - terminal workspace composition.
- `src/components/TerminalPane.tsx` - xterm terminal pane.
- `src/components/HostKeyTrustDialog.tsx` - host-key trust and mismatch UI.

## Current Capabilities

- Create, edit, delete, and select SSH connections.
- Start, close, and track terminal sessions.
- Hydrate session history and append live terminal events.
- Split terminal panes, search, copy logs, and export redacted logs.
- Prompt for host-key trust and show mismatch errors.

## Known Gaps

- Release readiness belongs in `docs/release/*` and `docs/testing/*`.
- Real SSH behavior requires manual verification against a reachable SSH server.

## Test / Verify

- `pnpm test -- packages/ssh-terminal/src/model/terminal-state.test.ts packages/ssh-terminal/src/model/errors.test.ts`
- `pnpm run build`
- For behavior changes, manually verify password/key auth, PTY input/output,
  resize, search, log export, host-key trust, and reconnect against a live SSH
  server.
