# API Debugger Postman-like Workbench Design

## Goal

Refactor the API Debugger into the interaction sample page defined by
`docs/ui/interaction-guidelines.md`, while preserving all existing backend calls,
request execution behavior, persistence behavior, history behavior, import/export
behavior, and security boundaries.

## Scope

This change is a frontend interaction and layout refactor inside
`packages/api-client`, plus the minimum desktop composition changes needed to
connect the sidebar to the feature-owned tab workspace.

The implementation must not change:

- Tauri commands or Rust code.
- The Command Bus protocol.
- `sendApiRequest`, `saveApiRequest`, history, environment, import, export,
  duplicate, or delete semantics.
- Credential storage or sensitive-data redaction.
- Package dependency direction.

No new dependency will be added.

## Architecture

`ApiDebuggerPage` remains the feature composition root. It combines the request
tab bar, primary request bar, request configuration area, response area, save
and close-confirmation dialogs, and workspace-level split direction.

A feature-owned `useApiRequestTabs` state model manages the open request
contexts. Each request tab owns its draft, response, request sub-tab, response
sub-tab, persistence baseline, execution status, and operation errors.

Existing command-client functions remain the only business-operation boundary.
The new state layer coordinates those calls but does not replace or rewrite
them.

The desktop sidebar remains a shell mounting surface. API-specific tree
construction, history loading, context menus, and request actions stay in
`packages/api-client`.

## Request Tab Model

Each open tab has a stable local id and one source:

- `new`: a newly created request without a persisted id.
- `saved`: a saved collection request.
- `history`: a request loaded from an API history detail.

Each tab stores:

- Source id and persisted request id when applicable.
- Request draft: name, folder, method, URL, headers, query, body, and environment
  variables needed by the current UI.
- Normalized saved baseline, or `null` for never-saved objects.
- Current response and send error.
- Request configuration tab: Params, Auth, Headers, or Body.
- Response tab: Body, Headers, Cookies, or Timing.
- Save state: Unsaved, Saved, Dirty, or Saving.
- Execution state: idle, Sending, Success, or Failed.

Saved requests are unique by persisted request id. Selecting an already-open
saved request activates its tab. History entries are unique by history id.
Selecting an already-open history entry activates its tab. New requests always
create a new tab.

Closing the active tab activates the nearest remaining tab. When no tabs remain,
the page shows a lightweight no-request state with a New Request action.

## Dirty and Saved Semantics

A normalized business-input snapshot defines the persistence baseline.
Normalization includes the persisted request fields and excludes response,
active sub-tabs, operation messages, and React object identity.

- A saved request matching its baseline is Saved.
- A saved request differing from its baseline is Dirty.
- A new or history request without a successful save is Unsaved.
- Starting a save sets Saving without changing the baseline.
- A successful save updates the persisted id and normalized baseline, then sets
  Saved.
- A failed save preserves Dirty or Unsaved and displays the error near Save.
- Send never changes the persistence state.

## Opening Behavior

Collection and History nodes use a stable open-or-activate rule. There is no
preview or pinned-tab model in this phase.

- Clicking a Collection request opens or activates its unique saved-request tab.
- Clicking a History item loads its detail and opens or activates an Unsaved
  history tab.
- New Request creates an `Untitled Request` tab.
- Switching tabs preserves request input, response, scroll-capable panel
  content, and active request/response sub-tabs.

## Layout

The API module uses the existing application sidebar rather than creating a
feature-specific outer shell.

The main workspace contains:

1. Request Tabs.
2. Primary Request Bar: Method, URL, Send, Save, More.
3. Request configuration: Params, Auth, Headers, Body.
4. Response: Body, Headers, Cookies, Timing, with persistent status, duration,
   and size.

The default request/response split is vertical in page flow: request above,
response below. A control in the response area's lower-right corner switches
between top/bottom and left/right arrangements.

Split direction is shared at Workspace scope for the API module. Switching
direction only recomposes the panels; it must not recreate request tabs or
reset drafts, responses, or active sub-tabs. Persistence across application
restarts is not required in this phase unless an existing layout persistence
hook can store the value without changing a public contract.

## Sidebar Tree

`ApiCollectionTree` becomes a unified feature-owned navigation surface using
the shared `TreeView`.

Sections:

- Collections, grouped by folder.
- Environments, retaining the current environment entry point and empty state.
- History, grouped by Today, Yesterday, Previous 7 Days, or explicit date.

Request rows display method and name. History rows display method and a compact
URL or request name. Full URLs are available through tooltips and context-menu
copy actions.

Collection and History queries continue to use their existing query keys and
command-client functions.

## Context Menus

Collection request menus expose:

- Open.
- Open in New Tab, disabled because Open already uses a unique object tab.
- Send.
- Rename, disabled because this phase does not add a rename-specific flow.
- Duplicate.
- Copy URL.
- Export.
- Delete.

History menus expose:

- Open.
- Open in New Tab, disabled for the same unique-tab reason.
- Save as Request.
- Copy URL.
- Delete from History, disabled because no backend delete capability exists.

Working menu entries call the same handlers used by visible controls. Disabled
entries must visibly communicate that the capability is not implemented.
Deletion keeps the existing backend operation. A confirmation dialog may be
added around it without changing the mutation.

## Primary Request Bar

The first visual layer contains:

- Compact Method select.
- Flexible URL input.
- Primary Send button.
- Save button.
- More menu for secondary actions.

Name and Folder are removed from the permanent editor surface.

- Existing saved requests save directly over the current persisted request.
- New and History requests open a lightweight Save dialog containing Name and
  Folder before calling the existing save operation.
- `Ctrl/Cmd+Enter` sends the active request.
- `Ctrl/Cmd+S` saves the active request.
- Sending disables duplicate submission. Cancel is not shown because the
  existing backend does not expose request cancellation.

## Request Configuration

The tab order is Params, Auth, Headers, Body.

- Params reuses the current query key/value behavior.
- Headers reuses the current header key/value behavior.
- Auth continues to expose the existing environment-variable workflow rather
  than inventing a new persisted authentication protocol.
- Body continues to use Monaco and keeps the current request-body rules.

The tab count shows enabled items or body presence. Existing sensitive-value
masking and duplicate-environment-key feedback remain.

This phase may keep the key/value editor feature-local because the repository
requires two confirmed consumers before moving a component into `packages/ui`.

## Response Area

The Response area always exposes Body, Headers, Cookies, and Timing.
History is removed from the response-area top-level switch and lives only in
the sidebar.

The response header continuously displays available status, duration, and size.
The area distinguishes:

- No request sent yet.
- Sending.
- Successful response with content.
- Successful response with an empty body.
- HTTP failure response.
- Network failure or timeout with recoverable error details.

Response body formatting, JSON detection, long-response notice, header display,
cookie extraction, and timing calculations retain their current behavior.
The page-local EmptyState is replaced with the shared state primitives.

## Close Confirmation

Saved and unchanged tabs close immediately.

Dirty and Unsaved tabs open a feature-owned confirmation dialog with:

- Save.
- Don't Save.
- Cancel.

Save follows the normal save rules. New and History tabs may require the
Name/Folder Save dialog before the close can complete. Save failures leave the
tab open and preserve all input.

## Component Plan

Expected new feature components:

- `ApiRequestTabs`: renders and closes request object tabs.
- `ApiRequestBar`: renders Method, URL, Send, Save, and More.
- `ApiSaveDialog`: collects Name and Folder for new/history requests.
- `ApiCloseRequestDialog`: resolves dirty/unsaved close behavior.
- `ApiHistoryTree`: maps history records into tree groups.
- `ApiWorkspaceLayoutToggle`: switches top/bottom and left/right layouts.

Expected state/model additions:

- `useApiRequestTabs`: owns multi-request state and active-tab lifecycle.
- Pure request-tab reducer and normalization helpers for deterministic testing.

Existing components may be narrowed or renamed where their old responsibility
no longer matches the target layout. No generic API business state will move
to `packages/ui`.

## Testing

Test-driven implementation starts with pure state/model tests covering:

- Opening and activating unique saved-request tabs.
- Opening and activating unique history tabs.
- Creating multiple independent new-request tabs.
- Independent drafts, responses, and active sub-tabs.
- Saved-to-Dirty transitions from normalized input changes.
- Send not changing persistence status.
- Successful save updating id and baseline.
- Failed save retaining Dirty or Unsaved.
- Close selection and confirmation decisions.
- Workspace-level split-direction switching without tab-state loss.
- History date grouping and tree mapping.

The current repository does not include React Testing Library. Component
composition will therefore be verified through TypeScript/build checks and an
in-app browser first-viewport inspection, while behavior-heavy logic remains in
pure tested functions.

Required verification:

- Focused API Debugger tests.
- Full `pnpm run test`.
- `git diff --check`.
- `pnpm run build`.
- `pnpm run check:rust`.
- `pnpm run check:rust:ssh`.
- `pnpm run test:rust`.
- Local app first-viewport inspection for the API Debugger.

## Compatibility and Risks

Primary risks:

- Existing desktop selection state is a single request id; composition must
  avoid making the shell own feature tab state.
- Multiple simultaneous send/save mutations need tab-local result routing so a
  completion cannot update the wrong active tab.
- Saved-request refreshes must not overwrite an open Dirty draft.
- History detail loading is asynchronous and must activate the intended tab
  even if the user changes tabs before completion.
- Monaco instances and large responses may increase memory use when many tabs
  are open. Only the active tab's heavy editor/viewer should be mounted.
- Shared `TreeView`, `Tabs`, and `ContextMenu` have known keyboard limitations.
  This phase uses their current public APIs and records unsupported interactions
  rather than broadening the shared-component refactor.

The working tree already contains unrelated changes in
`apps/desktop/src-tauri/Cargo.toml` and `pnpm-lock.yaml`. They must remain
untouched by this work.
