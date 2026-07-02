# Interaction Guidelines

This document defines Unfour's workbench interaction model. It is a stable
interaction guide, not a refactor plan or progress audit.

## Product Interaction Model

Unfour is object-first:

- users select requests, connections, tables, queries, and terminal sessions
  from trees, tabs, history, or search;
- editing forms modify selected objects but should not become the primary
  navigation model;
- open work should stay visible through tabs, split panes, status badges, and
  persistent panel state.

Primary actions:

- API Client: Send.
- Database: Run SQL or open table data.
- SSH Terminal: Connect or start session.

Each action region should visually emphasize one primary action. Secondary and
low-frequency actions belong in menus, inspectors, context menus, or the
command palette.

## Common Page Structure

Modules should compose the shared shell instead of creating independent app
shells:

```text
Global Toolbar
├─ Sidebar
│  └─ Resource tree or navigation
├─ Main Workspace
│  ├─ Workbench tabs
│  └─ Main panel
│     ├─ Module toolbar
│     ├─ Primary editor or viewer
│     └─ Optional results/messages area
├─ Inspector, optional and contextual
└─ Status Bar
```

## Trees

- Single-click selects a node.
- Expand controls change expansion only.
- Double-click opens a leaf node or toggles a parent node.
- Selected, focused, hover, disabled, loading, and error states must be visually
  distinct.
- Node IDs must be stable and must not use mutable array indexes.
- Refresh should preserve expansion and selection for nodes that still exist.
- Context-menu actions and row action buttons should call the same business
  handlers.
- Do not embed full forms inside tree rows.

Keyboard expectations:

- Arrow Up/Down moves between visible nodes.
- Arrow Right expands a parent or moves to the first child.
- Arrow Left collapses a parent or moves to the parent.
- Home/End moves to the first or last visible node.
- Enter activates the default action.
- Space selects without activating.
- Shift+F10 or Menu opens the context menu.

## Context Menus

- Context menus provide object actions, but must not be the only entry point for
  critical actions.
- Order actions as: Open/Run/Connect, New/Duplicate, Copy/Export, Refresh,
  Settings, Dangerous Actions.
- Use separators between action groups.
- Dangerous actions go last and use danger tone.
- Do not show unimplemented placeholder actions.
- Disabled actions need a clear reason.
- Menu shortcuts displayed in the UI must actually work.
- Focus should move into the menu when it opens and return to the trigger when
  it closes.

## Workbench Tabs

Tabs represent active work objects, such as requests, SQL queries, table views,
or terminal sessions.

Required tab states:

- active;
- focused;
- dirty;
- unsaved;
- loading or running;
- error, disconnected, or closed where applicable.

Rules:

- Dirty tabs must use a stable marker, not title text hacks.
- Closing a dirty tab requires Save / Don't Save / Cancel when saving is
  supported.
- Closing a running session or execution should explain whether work continues
  or stops.
- Tab overflow must remain readable through scrolling or an overflow menu.
- Module-level entry tabs and object-level tabs must not express the same
  hierarchy twice.

## Split Panes

- Support horizontal and vertical orientations where needed.
- Handles should have a comfortable hit area even if the visual line is small.
- Panes need minimum sizes so the user can recover from resizing.
- Double-click reset and keyboard resizing are preferred when feasible.
- Collapsing should preserve the previous non-collapsed size.
- Layout state belongs in composition or workspace state, not feature business
  services.

## Toolbars

- One toolbar should highlight one primary action.
- Left side: context, object identity, and navigation.
- Right side: execution and view actions.
- Action order must remain stable during pending states.
- Opposite actions should occupy the same position, such as Run/Stop or
  Connect/Disconnect.
- Icon-only actions require accessible labels and tooltips.
- Narrow layouts should hide text labels before hiding the primary action.
- Pending actions must prevent duplicate submission.

## Empty, Loading, Error, And Success

Empty states must distinguish:

- no object exists;
- no object is selected;
- result is empty;
- filter found no matches.

Loading rules:

- Initial load may use an area loading state.
- Refresh should keep existing content visible with local progress.
- Layout should stay stable while loading.

Error rules:

- Errors belong near the failed operation.
- Provide retry, settings, or copy-details actions where useful.
- Do not rely only on the status bar or a toast.

Success rules:

- Save, copy, refresh, and similar successes can use short inline feedback.
- Persistent success states such as Connected or Saved should use stable status
  text or badges.

## Dirty, Saved, And Unsaved

Definitions:

- Saved: current content matches the most recent successful persistent snapshot.
- Dirty: a saved object has local changes.
- Unsaved: an object has never been persisted or came from history/temporary
  output.

Rules:

- Send, Run, and Connect are not Save.
- Save only updates the baseline after the mutation succeeds.
- Save failure keeps the object dirty or unsaved.
- Dirty comparison should use normalized business input, not object identity.
- Switching tabs or modules must preserve dirty content.
- Auto-save is appropriate for low-risk layout preferences, not requests, SQL,
  or connection credentials by default.

## Keyboard Shortcuts

Shell-level shortcuts should be registered centrally. Active modules may add
contextual commands. Inputs, Monaco, and xterm keep priority over text-editing
shortcuts.

Recommended defaults:

| Action | Windows/Linux | macOS | Scope |
| --- | --- | --- | --- |
| Command Palette | `Ctrl+Shift+P` | `Cmd+Shift+P` | Global |
| Quick Open | `Ctrl+P` | `Cmd+P` | Global |
| Save | `Ctrl+S` | `Cmd+S` | Active object |
| Close Tab | `Ctrl+W` | `Cmd+W` | Active tab |
| Reopen Closed Tab | `Ctrl+Shift+T` | `Cmd+Shift+T` | Workbench |
| Toggle Sidebar | `Ctrl+B` | `Cmd+B` | Global |
| Toggle Bottom Panel | `Ctrl+J` | `Cmd+J` | Global |
| Send API Request | `Ctrl+Enter` | `Cmd+Enter` | API Client |
| Run SQL | `Ctrl+Enter` | `Cmd+Enter` | Database |
| Run Selected SQL | `Ctrl+Shift+Enter` | `Cmd+Shift+Enter` | Database |
| New SSH Session | `` Ctrl+Shift+` `` | `` Cmd+Shift+` `` | SSH Terminal |
| Clear Terminal | `Ctrl+L` | `Ctrl+L` | SSH Terminal when xterm permits |
| Open Context Menu | `Shift+F10` | `Shift+F10` | Focused object |

## Dangerous Operation Confirmation

Use three levels:

| Level | Examples | Expected handling |
| --- | --- | --- |
| Low impact or reversible | Clear filter, close clean tab | Execute directly and provide undo or recovery when practical. |
| Local irreversible | Delete saved request, delete connection, close active SSH session | Confirmation dialog with object name and impact. |
| High impact | Delete workspace, execute mutation SQL, trust changed host key | Confirmation dialog with impact summary, target, and explicit irreversible warning. |

Danger buttons use danger tone. Default focus should stay on Cancel for
high-impact operations. Do not use `window.confirm`.

## API Client Expectations

- The primary request bar should keep method, URL, Send, Save, and More close
  together.
- Params, Auth, Headers, and Body should be organized as request configuration
  tabs.
- Response views should include Body, Headers, Cookies, Timing, status, size,
  and duration.
- Sending errors should appear in the response area with enough detail to act.
- History replay creates an unsaved preview until saved.
- Saved request deletion must be explicit and should preserve dirty-state
  semantics.

## Database Expectations

- The sidebar should focus on connection/schema/table navigation, not long
  connection forms.
- SQL editor tabs and table tabs should preserve their own draft/result state.
- Mutation, schema-change, transaction-control, and unknown SQL must continue
  to follow backend confirmation policy.
- Results are read-only unless table editing is explicitly scoped.
- Copy/export must preserve clear handling for NULL, empty strings, binary, and
  object-like values.
- Query messages should include status, duration, affected rows when available,
  and actionable error detail.

## SSH Terminal Expectations

- Connections and active sessions should have clear, separate states.
- New Session and Reconnect must not be treated as the same interaction.
- Terminal tabs should show the most important state without hiding the title.
- Closing connected sessions requires confirmation.
- Copy Logs and Export Logs must use redacted session logs.
- Clear Terminal clears the visible buffer only; it must not imply deletion of
  persisted history unless such behavior is explicitly added.
- Split panes need clear focus ownership so toolbar actions target the expected
  session.

## Workspace And Settings Expectations

- Workspace switching and management should remain lightweight.
- Mutating dialogs should stay open until the mutation succeeds, or keep the
  user's input and show the error when it fails.
- Workspace deletion should explain local consequences and affected resources
  when counts are available.
- Low-frequency settings should not occupy the default main work surface.

## Ownership

- General, business-free components with multiple consumers belong in
  `packages/ui`.
- API request state belongs in `packages/api-client`.
- SQL execution and database view state belongs in `packages/database`.
- SSH session state belongs in `packages/ssh-terminal`.
- Shell composition belongs in `packages/app-shell` and `apps/desktop`.
- Backend execution and security policy belong behind the Rust command bus.
