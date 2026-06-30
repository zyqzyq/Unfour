// @vitest-environment jsdom
import type { ApiRequestTab } from "../model/request-tabs";
import { cleanup, render, screen } from "@testing-library/react";
import { I18nProvider } from "@unfour/ui";
import { afterEach, describe, expect, it, vi } from "vitest";
import { ApiRequestTabs } from "./ApiRequestTabs";

function requestTab(overrides: Partial<ApiRequestTab> = {}): ApiRequestTab {
  return {
    baseline: null,
    draft: {
      auth: { type: "none" },
      body: "",
      bodyMode: "none",
      collectionId: null,
      envVariables: [],
      formBody: [],
      headers: [],
      method: "GET",
      name: "Untitled Request",
      parentFolderId: null,
      query: [],
      rawBodyType: "json",
      url: "https://api.example.com/resource",
    },
    id: "new:1",
    lastRequest: null,
    requestTab: "query",
    response: null,
    responseTab: "body",
    saveError: null,
    savedRequestId: null,
    saving: false,
    sendError: null,
    sending: false,
    source: "new",
    sourceId: null,
    ...overrides,
  };
}

afterEach(() => {
  cleanup();
});

describe("ApiRequestTabs", () => {
  it("keeps new-request and environment controls outside the scrollable tab list", () => {
    const { container } = render(
      <I18nProvider initialLocale="en">
        <ApiRequestTabs
          activeId="new:1"
          endControl={<button type="button">Active environment</button>}
          onClose={vi.fn()}
          onCloseAll={vi.fn()}
          onCloseLeft={vi.fn()}
          onCloseRight={vi.fn()}
          onCloseSaved={vi.fn()}
          onNew={vi.fn()}
          onSelect={vi.fn()}
          tabs={[
            requestTab(),
            requestTab({ id: "new:2", draft: { ...requestTab().draft, name: "Second" } }),
          ]}
        />
      </I18nProvider>,
    );

    const tablist = screen.getByRole("tablist");

    expect(screen.getByRole("button", { name: "New Request" })).toBeInTheDocument();
    expect(screen.getByRole("button", { name: "Active environment" })).toBeInTheDocument();
    expect(tablist).not.toContainElement(screen.getByRole("button", { name: "New Request" }));
    expect(tablist).not.toContainElement(screen.getByRole("button", { name: "Active environment" }));
    // The new-request button sits immediately after the scrollable tab list so it
    // stays adjacent to the tabs, while the environment control stays pinned last.
    expect(tablist.nextElementSibling).toBe(
      screen.getByRole("button", { name: "New Request" }),
    );
    expect(container.firstElementChild?.lastElementChild).toContainElement(
      screen.getByRole("button", { name: "Active environment" }),
    );
  });
});
