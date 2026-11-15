# ssh-terminal

## Purpose

`@unfour/ssh-terminal` owns the SSH Terminal frontend experience.

## Local constraints

See [AGENTS.md](AGENTS.md) for this package's scope and invariants.

## Key Files

- `src/TerminalPage.tsx` - page/dialog composition, workspace lifecycle,
  Shell Sidebar integration, and search keyboard shortcut.
- `src/hooks/useSshConnectionManagement.ts` - connection drafts, save/test
  mutations, and host-key trust/mismatch/reset UI coordination.
- `src/hooks/useTerminalSessionOrchestration.ts` - session queries, failure
  merging, stable tab order, channel/history wiring, and session mutations.
- `src/hooks/useTerminalSessionActions.ts` - close/reconnect confirmations,
  batch close, and associated SFTP state removal.
- `src/model/terminal-state.ts` - terminal event store and log redaction.
- `src/model/ssh-connection-state.ts` - connection form defaults/conversion.
- `src/model/terminal-tabs.ts` - session tab derivation.
- `src/model/sftp-state.ts` - per-session Remote Files state and persisted panel width.
- `src/components/TerminalWorkspace.tsx` - terminal workspace composition.
- `src/components/SftpWorkspace.tsx` - lazy Remote Files panel, edge handle, and resize boundary.
- `src/components/TerminalPane.tsx` - xterm terminal pane.
- `src/components/TerminalContextMenu.tsx` - clipboard and selection actions.
- `src/components/TerminalLogPanel.tsx` - log display, copy, and export UI.
- `src/components/HostKeyTrustDialog.tsx` - host-key trust and mismatch UI.

## State and Lifecycle Boundaries

TanStack Query owns backend connection/session data through `useSshConnections`
and `useTerminalSessions`. Zustand retains workspace-scoped terminal slices
(events, selection, frontend-only failures, dismissed sessions, and search/split
state) and per-session SFTP UI state. Connection drafts and dialog state stay
local to `useSshConnectionManagement`; selected connection state continues to
use the documented `workspace-core` boundary.

Connection management preserves render-time form adjustment: clean drafts
refresh from external updates, while dirty drafts retain their contents and
require conflict resolution. Save callbacks update the originating query cache
before checking the page-owned workspace scope for selection/dialog changes.
Host-key handling here coordinates the existing UI and command-client calls;
verification remains in the backend.

Session orchestration reuses `useTerminalSessions`, `useSshTerminalChannel`,
`useActiveSessionHistory`, `useTerminalSessionActions`, and `model/terminal-tabs`.
It distinguishes frontend-only failures from backend sessions and orders visible
tabs by creation time/session ID, independent of polling order. The page keeps
workspace activation before active-session reconciliation and maintains the
mutation scope guard. Sidebar callbacks retain stable identities, and the page
owns sidebar installation/cleanup and keyboard listener activation.

While the Connections surface is hidden, its polling and history hydration stop
and its terminal-event snapshot freezes. The channel remains registered so the
store continues receiving output; showing the surface resumes the live snapshot.
Public component and command interfaces are unchanged. Log export UI remains in
`TerminalLogPanel`, with existing redaction boundaries preserved.

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
