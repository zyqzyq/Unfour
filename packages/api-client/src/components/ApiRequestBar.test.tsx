// @vitest-environment jsdom
import type { ApiRequestTab } from "../model/request-tabs";
import { cleanup, render, screen } from "@testing-library/react";
import { I18nProvider } from "@unfour/ui";
import { afterEach, describe, expect, it, vi } from "vitest";
import { ApiRequestBar } from "./ApiRequestBar";

function requestTab(): ApiRequestTab {
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
  };
}

afterEach(() => {
  cleanup();
});

describe("ApiRequestBar", () => {
  it("keeps request controls focused and leaves environment switching to the tab bar", () => {
    render(
      <I18nProvider initialLocale="en">
        <ApiRequestBar
          onSave={vi.fn()}
          onSend={vi.fn()}
          onUpdate={vi.fn()}
          tab={requestTab()}
        />
      </I18nProvider>,
    );

    expect(screen.getByRole("button", { name: "Send" })).toBeInTheDocument();
    expect(screen.queryByRole("button", { name: "Active environment" })).toBeNull();
    expect(screen.queryByRole("button", { name: "Request actions" })).toBeNull();
  });
});
