# API Debugger Postman-like Workbench Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Build a real multi-request API Debugger workbench with Collection/History navigation, request tabs, Postman-like request and response panels, explicit persistence/execution states, and workspace-level split direction without changing existing backend operations.

**Architecture:** Keep deterministic tab lifecycle and dirty-state logic in pure feature-owned model functions, then wrap it with a React hook that routes existing command-client mutations back to the originating tab. Keep the desktop shell thin: it publishes sidebar open intents, while `packages/api-client` owns request state, trees, menus, dialogs, and workbench composition.

**Tech Stack:** React 19, TypeScript, TanStack Query, Vitest, Monaco Editor, existing `@unfour/ui` primitives, existing `@unfour/command-client` API functions.

---

## File Map

- Create `packages/api-client/src/model/request-tabs.ts` — pure tab constructors, normalization, reducer-style updates, grouping, and close-selection logic.
- Create `packages/api-client/src/model/request-tabs.test.ts` — TDD coverage for multi-tab state, dirty baselines, save/send isolation, history grouping, and layout changes.
- Create `packages/api-client/src/hooks/useApiRequestTabs.ts` — query/mutation coordination for the pure tab model.
- Create `packages/api-client/src/components/ApiRequestTabs.tsx` — object tab strip and tab status indicators.
- Create `packages/api-client/src/components/ApiRequestBar.tsx` — Method + URL + Send + Save + More.
- Create `packages/api-client/src/components/ApiSaveDialog.tsx` — Name/Folder save flow.
- Create `packages/api-client/src/components/ApiCloseRequestDialog.tsx` — Save / Don't Save / Cancel.
- Create `packages/api-client/src/components/ApiWorkspaceLayoutToggle.tsx` — workspace-level top/bottom vs left/right control.
- Create `packages/api-client/src/components/ApiHistoryTree.tsx` — history grouping and tree item construction.
- Modify `packages/api-client/src/ApiDebuggerPage.tsx` — compose the workbench.
- Modify `packages/api-client/src/components/ApiCollectionTree.tsx` — shared TreeView, Collection and History sections, context menus.
- Modify `packages/api-client/src/components/ApiRequestEditor.tsx` — remove Name/Folder and host request configuration only.
- Modify `packages/api-client/src/components/RequestParamsTabs.tsx` — Params/Auth/Headers/Body order and active-tab state.
- Modify `packages/api-client/src/components/ApiResponseViewer.tsx` and `ResponseTabs.tsx` — response-only states and layout toggle.
- Modify `packages/api-client/src/components/RequestActionsMenu.tsx` — active request secondary actions.
- Modify `packages/api-client/src/model/types.ts`, `request-utils.ts`, `index.ts` — new model types, helpers, exports.
- Modify `apps/desktop/src/App.tsx` and `apps/desktop/src/components/ModuleSidebar.tsx` — sidebar open intent bridge only.

### Task 1: Pure Request Tab State Model

**Files:**
- Create: `packages/api-client/src/model/request-tabs.test.ts`
- Create: `packages/api-client/src/model/request-tabs.ts`
- Modify: `packages/api-client/src/model/types.ts`
- Modify: `packages/api-client/src/request-utils.ts`

- [ ] **Step 1: Write failing state-model tests**

Add Vitest cases that call the wished-for API:

```ts
const first = openSavedRequest(emptyApiTabsState("ws-1"), savedA);
const second = openSavedRequest(first, savedA);
expect(second.tabs).toHaveLength(1);
expect(second.activeTabId).toBe(first.tabs[0].id);

const dirty = updateTabDraft(first, first.activeTabId!, { url: "https://changed.test" });
expect(getTabSaveState(dirty.tabs[0])).toBe("dirty");

const history = openHistoryRequest(first, historyDetail);
expect(getTabSaveState(history.tabs.at(-1)!)).toBe("unsaved");

const sent = completeTabSend(startTabSend(dirty, dirty.activeTabId!), dirty.activeTabId!, response);
expect(getTabSaveState(sent.tabs[0])).toBe("dirty");

const saved = completeTabSave(dirty, dirty.activeTabId!, savedA);
expect(getTabSaveState(saved.tabs[0])).toBe("saved");

const horizontal = setApiSplitDirection(saved, "horizontal");
expect(horizontal.splitDirection).toBe("horizontal");
expect(horizontal.tabs).toEqual(saved.tabs);
```

- [ ] **Step 2: Run the focused test and verify RED**

Run:

```bash
pnpm vitest run packages/api-client/src/model/request-tabs.test.ts
```

Expected: FAIL because `request-tabs.ts` and its exports do not exist.

- [ ] **Step 3: Implement the minimal pure model**

Define:

```ts
export type ApiRequestTab = {
  id: string;
  source: "new" | "saved" | "history";
  sourceId: string | null;
  savedRequestId: string | null;
  draft: RequestDraft;
  baseline: string | null;
  response: ApiResponse | null;
  sendError: string | null;
  sending: boolean;
  saving: boolean;
  saveError: string | null;
  requestTab: RequestParamsTab;
  responseTab: ResponseTab;
};

export type ApiTabsState = {
  workspaceId: string;
  tabs: ApiRequestTab[];
  activeTabId: string | null;
  splitDirection: "vertical" | "horizontal";
};
```

Implement deterministic constructors and immutable update helpers. Normalize
`name`, `folderPath`, `method`, `url`, `headers`, `query`, `body`, `bodyKind`,
and `timeoutMs` with `JSON.stringify`. Use stable ids `saved:<id>` and
`history:<id>`; use `new:<counter-or-random>` only for new requests.

Implement `groupApiHistory(items, now)` returning Today, Yesterday,
Previous 7 Days, then ISO date groups.

- [ ] **Step 4: Run focused tests and verify GREEN**

Run:

```bash
pnpm vitest run packages/api-client/src/model/request-tabs.test.ts
```

Expected: PASS.

### Task 2: Command Coordination Hook

**Files:**
- Create: `packages/api-client/src/hooks/useApiRequestTabs.ts`
- Modify: `packages/api-client/src/hooks/useApiHistory.ts`
- Modify: `packages/api-client/src/hooks/useApiRequest.ts`

- [ ] **Step 1: Add failing tests for any additional pure transition discovered by the hook**

Add tests proving stale async completions update the addressed tab id rather
than the active tab and that a refreshed saved query does not overwrite Dirty
drafts.

- [ ] **Step 2: Run focused tests and verify RED**

Run the request-tabs test file and confirm the new transition is missing.

- [ ] **Step 3: Implement `useApiRequestTabs`**

The hook must:

```ts
const savedQuery = useQuery({
  queryKey: ["api-saved", workspaceId],
  queryFn: () => listSavedApiRequests(workspaceId),
});
const historyQuery = useQuery({
  queryKey: ["api-history", workspaceId],
  queryFn: () => listApiHistory(workspaceId),
});
```

Route mutations with `{ tabId, input }` variables:

```ts
useMutation({
  mutationFn: ({ input }: { tabId: string; input: ApiRequestInput }) =>
    sendApiRequest(input),
  onSuccess: (response, variables) =>
    setState((current) => completeTabSend(current, variables.tabId, response)),
  onError: (error, variables) =>
    setState((current) => failTabSend(current, variables.tabId, formatError(error))),
});
```

Use the same addressed-tab pattern for save. Keep duplicate, delete,
environment save, import, and export calls unchanged. Expose open saved/history,
new, update draft, send, save, close request, active sub-tab setters, and
workspace split direction.

- [ ] **Step 4: Run focused tests and TypeScript build**

Run:

```bash
pnpm vitest run packages/api-client/src/model/request-tabs.test.ts
pnpm run build
```

Expected: PASS.

### Task 3: Request Workbench Components

**Files:**
- Create: `packages/api-client/src/components/ApiRequestTabs.tsx`
- Create: `packages/api-client/src/components/ApiRequestBar.tsx`
- Create: `packages/api-client/src/components/ApiSaveDialog.tsx`
- Create: `packages/api-client/src/components/ApiCloseRequestDialog.tsx`
- Create: `packages/api-client/src/components/ApiWorkspaceLayoutToggle.tsx`
- Modify: `packages/api-client/src/components/ApiRequestEditor.tsx`
- Modify: `packages/api-client/src/components/RequestParamsTabs.tsx`
- Modify: `packages/api-client/src/components/RequestActionsMenu.tsx`

- [ ] **Step 1: Add failing pure tests for tab labels and status derivation**

Test `requestTabTitle(tab)` and `requestTabVisualState(tab)` for Untitled,
Saved, Dirty, Sending, Success, and Failed.

- [ ] **Step 2: Run focused tests and verify RED**

Expected: FAIL because visual derivation helpers do not exist.

- [ ] **Step 3: Implement the components**

Use existing `Tabs`, `Button`, `Input`, `Dialog`, `Badge`, and menu primitives.
`ApiRequestTabs` maps `modified` to Dirty/Unsaved and `loading` to Sending.
`ApiRequestBar` keeps Send as the only primary action. `ApiSaveDialog` contains
Name and Folder. `ApiCloseRequestDialog` contains Save, Don't Save, and Cancel.
`ApiRequestEditor` contains only request configuration. The tab order is
Params, Auth, Headers, Body.

- [ ] **Step 4: Run focused tests and build**

Run:

```bash
pnpm vitest run packages/api-client/src/model/request-tabs.test.ts
pnpm run build
```

Expected: PASS.

### Task 4: Response States and Layout Composition

**Files:**
- Modify: `packages/api-client/src/components/ApiResponseViewer.tsx`
- Modify: `packages/api-client/src/components/ResponseTabs.tsx`
- Modify: `packages/api-client/src/ApiDebuggerPage.tsx`

- [ ] **Step 1: Add failing pure tests for response state derivation**

Cover idle, sending, success with body, success with empty body, HTTP failure,
network failure, and timeout.

- [ ] **Step 2: Run focused tests and verify RED**

Expected: FAIL for missing `deriveTabResponseState`.

- [ ] **Step 3: Implement response-only composition**

Remove History from the response viewer. Use shared `EmptyState`,
`LoadingState`, and `ErrorState`. Keep body formatting, headers, cookies,
timing, duration, and size behavior. Compose `SplitPane` with:

```tsx
orientation={splitDirection === "vertical" ? "vertical" : "horizontal"}
```

Default to request above response. Place `ApiWorkspaceLayoutToggle` in the
response lower-right without covering scrollable content.

- [ ] **Step 4: Run focused tests and build**

Run focused tests and `pnpm run build`; expect PASS.

### Task 5: Collection/History Tree and Desktop Bridge

**Files:**
- Create: `packages/api-client/src/components/ApiHistoryTree.tsx`
- Modify: `packages/api-client/src/components/ApiCollectionTree.tsx`
- Modify: `packages/api-client/src/index.ts`
- Modify: `apps/desktop/src/components/ModuleSidebar.tsx`
- Modify: `apps/desktop/src/App.tsx`

- [ ] **Step 1: Add failing tests for tree mapping and history grouping**

Assert folder nodes, request method metadata, Today/Yesterday/Previous 7 Days
groups, and stable ids.

- [ ] **Step 2: Run focused tests and verify RED**

Expected: FAIL until the tree mapping helper is implemented.

- [ ] **Step 3: Implement TreeView and open-intent bridge**

`ApiCollectionTree` uses shared `TreeView` and existing query keys. Add context
menus with working Open, Send, Duplicate, Copy URL, Export, Delete and disabled
unsupported entries. History uses `getApiHistoryDetail` through the feature
hook.

The desktop layer holds only a small API open intent:

```ts
type ApiOpenIntent =
  | { kind: "new"; nonce: number }
  | { kind: "saved"; requestId: string; nonce: number }
  | { kind: "history"; historyId: string; nonce: number };
```

Pass it to `ApiDebuggerPage`; do not move tab state into the shell.

- [ ] **Step 4: Run API tests and build**

Run:

```bash
pnpm vitest run packages/api-client
pnpm run build
```

Expected: PASS.

### Task 6: Full Verification and First Viewport

**Files:**
- Modify only files required to fix verification failures caused by this task.

- [ ] **Step 1: Run frontend verification**

```bash
pnpm run test
pnpm run lint
pnpm run build
git diff --check
```

Expected: PASS, or report unrelated baseline failures explicitly.

- [ ] **Step 2: Run Rust verification**

```bash
pnpm run check:rust
pnpm run check:rust:ssh
pnpm run test:rust
```

Expected: PASS without Rust changes.

- [ ] **Step 3: Inspect the API Debugger first viewport**

Start:

```bash
pnpm dev
```

Use the in-app browser to verify:

- Collection and History trees are visible.
- Request tabs are above Method + URL + Send + Save.
- Request tabs preserve independent drafts.
- Default split is top/bottom.
- Lower-right layout toggle switches to left/right without losing state.
- Request and response tabs use the required order.
- Dirty, saved, sending, success, failed, and empty-response states are visible.
- Context menus open and unsupported actions are disabled.

- [ ] **Step 4: Review final diff**

Confirm no changes to Rust, command-client contracts, package dependencies,
`apps/desktop/src-tauri/Cargo.toml`, or the user's existing `pnpm-lock.yaml`
changes.
