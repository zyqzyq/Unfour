// @vitest-environment jsdom
import type { ApiHistoryItem } from "@unfour/command-client";
import { I18nProvider } from "@unfour/ui";
import { cleanup, fireEvent, render, screen } from "@testing-library/react";
import { afterEach, describe, expect, it, vi } from "vitest";
import { ApiHistoryTree } from "./ApiHistoryTree";

const historyUrl =
  "http://192.168.20.50:30211/dataset/2082290710210600962/graph/schema/2082310418234269698";

function historyItem(overrides: Partial<ApiHistoryItem> = {}): ApiHistoryItem {
  return {
    id: "history-1",
    workspaceId: "ws-1",
    name: "Query current schema",
    method: "GET",
    url: historyUrl,
    status: 200,
    durationMs: 386,
    createdAt: "2026-06-15T05:43:00.000Z",
    updatedAt: "2026-06-15T05:43:00.000Z",
    deletedAt: null,
    revision: 1,
    syncStatus: "local",
    remoteId: null,
    ...overrides,
  };
}

afterEach(() => {
  cleanup();
});

function renderHistory(items: ApiHistoryItem[]) {
  const onOpenIntent = vi.fn();
  render(
    <I18nProvider initialLocale="en">
      <ApiHistoryTree items={items} onOpenIntent={onOpenIntent} />
    </I18nProvider>,
  );
  return onOpenIntent;
}

describe("ApiHistoryTree", () => {
  it("prioritizes a saved request name and keeps the full URL in the row tooltip", () => {
    const onOpenIntent = renderHistory([historyItem()]);

    expect(screen.getByText("Query current schema")).toBeInTheDocument();
    expect(screen.queryByText(historyUrl)).not.toBeInTheDocument();
    const row = screen
      .getByText("Query current schema")
      .closest("[role='treeitem']");
    expect(row).toHaveAttribute("title", `GET\n${historyUrl}`);

    fireEvent.click(screen.getByText("Query current schema"));
    expect(onOpenIntent).toHaveBeenCalledWith(
      expect.objectContaining({ historyId: "history-1", kind: "history" }),
    );
  });

  it("uses the pathname as the primary label when no request name exists", () => {
    renderHistory([historyItem({ name: null })]);

    expect(
      screen.getByText(
        "/dataset/2082290710210600962/graph/schema/2082310418234269698",
      ),
    ).toBeInTheDocument();
    expect(screen.queryByText(historyUrl)).not.toBeInTheDocument();
    expect(screen.getByText("200")).toBeInTheDocument();
  });
});
