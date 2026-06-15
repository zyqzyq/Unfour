import { describe, expect, it } from "vitest";
import type {
  ApiHistoryDetail,
  ApiHistoryItem,
  ApiResponse,
  ApiSavedRequest,
} from "@unfour/command-client";
import {
  completeTabSave,
  completeTabSend,
  createNewRequestTab,
  emptyApiTabsState,
  failTabSave,
  getTabSaveState,
  groupApiHistory,
  openHistoryRequest,
  openSavedRequest,
  setApiSplitDirection,
  startTabSave,
  startTabSend,
  updateTabDraft,
} from "./request-tabs";

describe("API request tab state", () => {
  it("opens or activates one tab per saved request", () => {
    const first = openSavedRequest(emptyApiTabsState("ws-1"), savedRequest("req-1"));
    const second = openSavedRequest(first, savedRequest("req-1"));

    expect(second.tabs).toHaveLength(1);
    expect(second.activeTabId).toBe("saved:req-1");
  });

  it("creates independent new request tabs", () => {
    const first = createNewRequestTab(emptyApiTabsState("ws-1"), "new:1");
    const second = createNewRequestTab(first, "new:2");
    const changed = updateTabDraft(second, "new:1", { url: "https://one.test" });

    expect(changed.tabs).toHaveLength(2);
    expect(changed.tabs.find((tab) => tab.id === "new:1")?.draft.url).toBe(
      "https://one.test",
    );
    expect(changed.tabs.find((tab) => tab.id === "new:2")?.draft.url).toBe("");
  });

  it("marks saved requests dirty using a normalized baseline", () => {
    const state = openSavedRequest(emptyApiTabsState("ws-1"), savedRequest("req-1"));
    const dirty = updateTabDraft(state, "saved:req-1", {
      headers: [{ enabled: true, key: "Accept", value: "application/json" }],
    });
    const restored = updateTabDraft(dirty, "saved:req-1", { headers: [] });

    expect(getTabSaveState(dirty.tabs[0])).toBe("dirty");
    expect(getTabSaveState(restored.tabs[0])).toBe("saved");
  });

  it("opens history as an unsaved unique tab", () => {
    const first = openHistoryRequest(
      emptyApiTabsState("ws-1"),
      historyDetail("history-1"),
    );
    const second = openHistoryRequest(first, historyDetail("history-1"));

    expect(second.tabs).toHaveLength(1);
    expect(second.activeTabId).toBe("history:history-1");
    expect(getTabSaveState(second.tabs[0])).toBe("unsaved");
  });

  it("keeps persistence state unchanged while sending", () => {
    const opened = openSavedRequest(emptyApiTabsState("ws-1"), savedRequest("req-1"));
    const dirty = updateTabDraft(opened, "saved:req-1", {
      url: "https://changed.test",
    });
    const sending = startTabSend(dirty, "saved:req-1");
    const completed = completeTabSend(sending, "saved:req-1", response());

    expect(sending.tabs[0].sending).toBe(true);
    expect(getTabSaveState(completed.tabs[0])).toBe("dirty");
    expect(completed.tabs[0].response?.status).toBe(200);
  });

  it("updates the addressed tab after an async send completes", () => {
    const first = createNewRequestTab(emptyApiTabsState("ws-1"), "new:1");
    const second = createNewRequestTab(first, "new:2");
    const completed = completeTabSend(
      startTabSend(second, "new:1"),
      "new:1",
      response(),
    );

    expect(completed.activeTabId).toBe("new:2");
    expect(completed.tabs.find((tab) => tab.id === "new:1")?.response?.status).toBe(200);
    expect(completed.tabs.find((tab) => tab.id === "new:2")?.response).toBeNull();
  });

  it("updates the saved baseline only after successful save", () => {
    const opened = createNewRequestTab(emptyApiTabsState("ws-1"), "new:1");
    const saving = startTabSave(opened, "new:1");
    const saved = completeTabSave(saving, "new:1", savedRequest("req-1"));

    expect(saving.tabs[0].saving).toBe(true);
    expect(saved.tabs[0].id).toBe("saved:req-1");
    expect(saved.tabs[0].savedRequestId).toBe("req-1");
    expect(getTabSaveState(saved.tabs[0])).toBe("saved");
  });

  it("retains unsaved or dirty state after failed save", () => {
    const opened = createNewRequestTab(emptyApiTabsState("ws-1"), "new:1");
    const failed = failTabSave(startTabSave(opened, "new:1"), "new:1", "Nope");

    expect(failed.tabs[0].saving).toBe(false);
    expect(failed.tabs[0].saveError).toBe("Nope");
    expect(getTabSaveState(failed.tabs[0])).toBe("unsaved");
  });

  it("switches workspace split direction without changing tab state", () => {
    const opened = createNewRequestTab(emptyApiTabsState("ws-1"), "new:1");
    const horizontal = setApiSplitDirection(opened, "horizontal");

    expect(horizontal.splitDirection).toBe("horizontal");
    expect(horizontal.tabs).toEqual(opened.tabs);
  });
});

describe("API history grouping", () => {
  it("groups recent history into stable date buckets", () => {
    const now = new Date("2026-06-15T12:00:00+08:00");
    const groups = groupApiHistory(
      [
        historyItem("today", "2026-06-15T08:00:00+08:00"),
        historyItem("yesterday", "2026-06-14T08:00:00+08:00"),
        historyItem("recent", "2026-06-10T08:00:00+08:00"),
        historyItem("older", "2026-05-01T08:00:00+08:00"),
      ],
      now,
    );

    expect(groups.map((group) => group.label)).toEqual([
      "Today",
      "Yesterday",
      "Previous 7 Days",
      "2026-05-01",
    ]);
    expect(groups[0].id).toBe("history:today");
  });
});

function savedRequest(id: string): ApiSavedRequest {
  return {
    id,
    workspaceId: "ws-1",
    name: "Health check",
    folderPath: "Examples",
    method: "GET",
    url: "https://example.test",
    headersJson: "[]",
    queryJson: "[]",
    body: null,
    bodyKind: "json",
    createdAt: "2026-06-15T00:00:00Z",
    updatedAt: "2026-06-15T00:00:00Z",
    deletedAt: null,
    revision: 1,
    syncStatus: "local",
    remoteId: null,
  };
}

function historyDetail(id: string): ApiHistoryDetail {
  return {
    id,
    workspaceId: "ws-1",
    name: "History request",
    method: "GET",
    url: "https://history.test",
    requestHeadersJson: "[]",
    requestQueryJson: "[]",
    requestBody: null,
    status: 200,
    durationMs: 12,
    responseHeadersJson: "[]",
    responseBodyPreview: "{}",
    createdAt: "2026-06-15T00:00:00Z",
    updatedAt: "2026-06-15T00:00:00Z",
    deletedAt: null,
    revision: 1,
    syncStatus: "local",
    remoteId: null,
  };
}

function response(): ApiResponse {
  return {
    historyId: "history-1",
    status: 200,
    statusText: "OK",
    headers: [],
    body: "{}",
    durationMs: 12,
  };
}

function historyItem(id: string, createdAt: string): ApiHistoryItem {
  return {
    id,
    workspaceId: "ws-1",
    name: id,
    method: "GET",
    url: `https://${id}.test`,
    status: 200,
    durationMs: 12,
    createdAt,
    updatedAt: createdAt,
    deletedAt: null,
    revision: 1,
    syncStatus: "local",
    remoteId: null,
  };
}
