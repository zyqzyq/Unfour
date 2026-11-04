// @vitest-environment jsdom
import type { ReactNode } from "react";
import { act, cleanup, renderHook, waitFor } from "@testing-library/react";
import { afterEach, expect, it, vi } from "vitest";
import { QueryClient, QueryClientProvider } from "@tanstack/react-query";
import { listSavedApiRequests, updateApiRequest, sendApiRequest, type ApiSavedRequest } from "@unfour/command-client";
import { useApiRequestTabs } from "./useApiRequestTabs";
import { resetApiRequestTabStore } from "../model/api-request-tab-state";

vi.mock("@unfour/command-client", async (original) => ({
  ...await original<typeof import("@unfour/command-client")>(),
  listSavedApiRequests: vi.fn(), listApiHistory: vi.fn().mockResolvedValue([]),
  listWorkspaceEnvironments: vi.fn().mockResolvedValue([]), updateApiRequest: vi.fn(), sendApiRequest: vi.fn(),
}));
afterEach(() => { cleanup(); resetApiRequestTabStore(); vi.clearAllMocks(); });
const saved: ApiSavedRequest = {
  id: "r", workspaceId: "w", name: "Original", collectionId: "c", parentFolderId: null,
  sortOrder: 0, authJson: '{"type":"none"}', method: "GET", url: "https://original.test",
  headersJson: "[]", queryJson: "[]", body: null, bodyKind: "none", settingsJson: "{}",
  preRequestScript: null, postResponseScript: null, scriptSchemaVersion: 1,
  createdAt: "1", updatedAt: "1", deletedAt: null, revision: 1, syncStatus: "local", remoteId: null,
};
async function setup() {
  vi.mocked(listSavedApiRequests).mockResolvedValue([saved]);
  const client = new QueryClient({ defaultOptions: { queries: { retry: false }, mutations: { retry: false } } });
  const hook = renderHook(() => useApiRequestTabs("w"), {
    wrapper: ({ children }: { children: ReactNode }) => <QueryClientProvider client={client}>{children}</QueryClientProvider>,
  });
  await waitFor(() => expect(hook.result.current.savedRequests).toHaveLength(1));
  act(() => hook.result.current.openSaved("r"));
  return { ...hook, client };
}

it("refreshes clean tabs, preserves dirty tabs across updates/deletes, and reloads explicitly without sending", async () => {
  const { result, client } = await setup();
  const latest = { ...saved, revision: 2, url: "https://external.test" };
  act(() => client.setQueryData(["api-saved", "w"], [latest]));
  await waitFor(() => expect(result.current.activeTab?.draft.url).toBe(latest.url));
  act(() => result.current.updateDraft("saved:r", { body: "unsaved body" }));
  const next = { ...latest, revision: 3, name: "External name" };
  act(() => client.setQueryData(["api-saved", "w"], [next]));
  await waitFor(() => expect(result.current.activeTab?.externalChange).toBe("updated"));
  expect(result.current.activeTab?.draft.body).toBe("unsaved body");
  await act(async () => { expect(await result.current.saveTab(result.current.activeTab!)).toBeNull(); });
  expect(updateApiRequest).not.toHaveBeenCalled();
  vi.mocked(listSavedApiRequests).mockResolvedValue([next]);
  await act(async () => { await result.current.reloadTab(result.current.activeTab!); });
  expect(result.current.activeTab?.draft.name).toBe("External name");
  expect(result.current.activeTab?.externalChange).toBeUndefined();
  act(() => client.setQueryData(["api-saved", "w"], []));
  await waitFor(() => expect(result.current.activeTab?.externalChange).toBe("deleted"));
  expect(result.current.activeTab?.draft.name).toBe("External name");
  expect(sendApiRequest).not.toHaveBeenCalled();
});

it("sends the original revision to reject an external edit that arrives before its refresh", async () => {
  const { result } = await setup();
  act(() => result.current.updateDraft("saved:r", { body: "local", bodyMode: "raw", rawBodyType: "text" }));
  vi.mocked(updateApiRequest).mockRejectedValue({ code: "ROW_CONFLICT", message: "changed" });
  await act(async () => { await result.current.saveTab(result.current.activeTab!); });
  expect(updateApiRequest).toHaveBeenCalledWith("w", "r", expect.objectContaining({ body: "local" }), 1);
  expect(result.current.activeTab?.draft.body).toBe("local");
  expect(result.current.activeTab?.saveError).toContain("changed outside this editor");
});

it("cancels a pre-save list read so its late response cannot downgrade the saved draft", async () => {
  const { result, client } = await setup();
  let finishOldRead!: (items: ApiSavedRequest[]) => void;
  vi.mocked(listSavedApiRequests).mockImplementationOnce(() => new Promise((resolve) => { finishOldRead = resolve; }));
  act(() => { void client.invalidateQueries({ queryKey: ["api-saved", "w"] }); });
  await waitFor(() => expect(finishOldRead).toBeDefined());
  const committed = { ...saved, name: "Saved edit", revision: 2 };
  vi.mocked(listSavedApiRequests).mockResolvedValue([committed]);
  vi.mocked(updateApiRequest).mockResolvedValue(committed);
  act(() => result.current.updateDraft("saved:r", { name: committed.name }));
  await act(async () => { await result.current.saveTab(result.current.activeTab!); });
  await act(async () => { finishOldRead([saved]); });
  expect(result.current.activeTab).toMatchObject({ savedRevision: 2, draft: { name: "Saved edit" } });
  expect(result.current.activeTab?.externalChange).toBeUndefined();
});

it("reconciles a refresh received during a save after that save fails", async () => {
  const { result, client } = await setup();
  let rejectSave!: (error: unknown) => void;
  vi.mocked(updateApiRequest).mockImplementationOnce(() => new Promise((_, reject) => { rejectSave = reject; }));
  act(() => result.current.updateDraft("saved:r", { name: "Local" }));
  let saving!: Promise<string | null>;
  act(() => { saving = result.current.saveTab(result.current.activeTab!); });
  await waitFor(() => expect(rejectSave).toBeDefined());
  const latest = { ...saved, name: "External", revision: 2 };
  vi.mocked(listSavedApiRequests).mockResolvedValue([latest]);
  act(() => client.setQueryData(["api-saved", "w"], [latest]));
  await act(async () => { rejectSave({ code: "ROW_CONFLICT" }); await saving; });
  await waitFor(() => expect(result.current.activeTab?.externalChange).toBe("updated"));
  expect(result.current.activeTab?.draft.name).toBe("Local");
});

it("keeps the draft when explicit reload fails instead of applying stale cached data", async () => {
  const { result } = await setup();
  act(() => result.current.updateDraft("saved:r", { name: "Local draft" }));
  vi.mocked(listSavedApiRequests).mockRejectedValueOnce(new Error("read failed"));
  await act(async () => { await result.current.reloadTab(result.current.activeTab!); });
  expect(result.current.activeTab?.draft.name).toBe("Local draft");
});
