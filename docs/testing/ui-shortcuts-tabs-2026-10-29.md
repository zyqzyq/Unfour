# UI shortcut and tab-close regression verification

Date: 2026-10-29 (Asia/Shanghai)
Base: local `main`, `78830dd3`.

## Scope and decisions

- API page shortcuts now yield to consumed events, IME composition (including
  key code 229), repeats, extra modifiers, and open dialogs/menus/popovers.
  Normal Ctrl/Cmd+Enter, Ctrl/Cmd+S and Ctrl/Cmd+L remain available.
- The request-name toolbar handles save in the bubble phase and respects
  composition and locally consumed events while preserving pending-name save.
- Removed unsupported Ctrl+W hints from shared, API and SSH tab menus. No new
  global close binding was introduced, avoiding conflicts with desktop/window
  and terminal keyboard handling.
- Shared Tabs confirms modified targets before a single or batch close. Its
  default close actions use one owner callback when supplied. The existing
  Dialog primitive supplies focus containment and Escape cancellation; the
  existing compact content and buttons remain.
- Database owns a sequential confirmation queue for SQL drafts and pending row
  edits. Cancel abandons the remaining queue. Running queries are skipped while
  other targets continue through their existing guards.
- API's existing save/discard/close queue and SSH's existing single/batch session
  confirmation callbacks are preserved. No backend request, SQL execution,
  persistence or SSH session implementation changed. No dependencies added.
- Shared Tabs, API close confirmation and API tab hover status use shared
  English/Chinese locale resources. API close cancellation restores focus to
  the invoking control or a surviving active request tab; transferring to the
  save dialog retains focus in that dialog.

## Automated results

PASS: 45 test files, 308 tests:

```powershell
pnpm exec vitest run packages/ui/src packages/api-client/src packages/database/src/components/DatabaseWorkspace.test.tsx packages/ssh-terminal/src/components/TerminalWorkspace.test.tsx packages/ssh-terminal/src/hooks/useTerminalSessionActions.test.tsx packages/ssh-terminal/src/components/SshTasksPage.test.tsx packages/ssh-terminal/src/components/SshTasksPage.states.test.tsx
```

Coverage includes Ctrl/Cmd+Enter inside save/close/external dialogs, context and
dropdown menus, popovers, consumed events, IME, inactive API pages, normal send
and save, modified single/batch close, cancel/Escape/focus restoration, save
before close, mixed running/clean/dirty database tabs, SSH owner callbacks,
and English/Chinese labels. Regressions were first observed failing before
the corresponding fixes; final runs pass.

- PASS: `pnpm run build` (includes TypeScript compilation).
- PASS: `pnpm --filter @unfour/desktop exec tsc --noEmit` during implementation.
- PASS: ESLint on all changed TypeScript/TSX files, zero errors. The existing
  `ApiClientPage` max-lines-per-function warning remains (427 lines).
- PASS: `git diff --check`.
- PASS: `pnpm run check:large-files`, zero blocking files. The first sandboxed
  invocation failed to start with `unable to open database file`; the approved
  retry outside the sandbox passed.
- Build warning: a minified bundle exceeds 500 kB; bundling changes are outside
  this interaction fix.

## Desktop verification still needed

NOT VERIFIED: native Tauri/WebView interactions on Windows/macOS, real IME
composition event ordering, native window Ctrl/Cmd+W handling, real Monaco
keybindings and focus traversal, xterm key delivery, screen-reader behavior,
and live API/SQL/SSH execution. Component tests use jsdom and isolated mocks;
they do not establish these native or live-service outcomes.

Manual follow-up: in the desktop app, exercise Ctrl/Cmd+Enter in the API editor
and each overlay, IME text entry, Escape/Tab/Shift+Tab in unsaved confirmations,
save/discard/cancel through multi-tab closes, and the Chinese/English menus.
Verify terminal keystrokes still reach xterm and running SQL tabs remain open.
