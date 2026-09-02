// @vitest-environment jsdom
import type { ApiRequestTab } from "../model/request-tabs";
import { cleanup, fireEvent, render, screen } from "@testing-library/react";
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
      preRequestScript: "",
      postResponseScript: "",
      formBody: [],
      headers: [],
      method: "GET",
      name: "",
      parentFolderId: null,
      query: [],
      rawBodyType: "json",
      url: "https://api.example.com/resource",
    },
    id: "new:1",
    execution: null,
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
          onNameCommit={vi.fn()}
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

  it("only enters name editing from the edit icon", () => {
    render(
      <I18nProvider initialLocale="en">
        <ApiRequestBar
          onNameCommit={vi.fn()}
          onSave={vi.fn()}
          onSend={vi.fn()}
          onUpdate={vi.fn()}
          tab={requestTab()}
        />
      </I18nProvider>,
    );

    fireEvent.click(screen.getByText("Untitled Request"));
    expect(screen.queryByRole("textbox", { name: "Request name" })).toBeNull();

    fireEvent.click(screen.getByRole("button", { name: "Edit name" }));
    expect(screen.getByRole("textbox", { name: "Request name" })).toBeInTheDocument();
  });

  it("updates the draft on Enter without persisting it", () => {
    const onNameCommit = vi.fn();
    const onSave = vi.fn();
    const onUpdate = vi.fn();
    render(
      <I18nProvider initialLocale="en">
        <ApiRequestBar
          onNameCommit={onNameCommit}
          onSave={onSave}
          onSend={vi.fn()}
          onUpdate={onUpdate}
          tab={requestTab()}
        />
      </I18nProvider>,
    );

    fireEvent.click(screen.getByRole("button", { name: "Edit name" }));
    const input = screen.getByRole("textbox", { name: "Request name" });
    fireEvent.change(input, { target: { value: "List users" } });

    expect(onNameCommit).not.toHaveBeenCalled();
    expect(onUpdate).not.toHaveBeenCalled();

    fireEvent.keyDown(input, { key: "Enter" });

    expect(onNameCommit).toHaveBeenCalledWith("List users");
    expect(onUpdate).not.toHaveBeenCalled();
    expect(onSave).not.toHaveBeenCalled();
  });

  it("cancels with Escape and commits the draft on blur", () => {
    const onNameCommit = vi.fn();
    const onSave = vi.fn();
    const { rerender } = render(
      <I18nProvider initialLocale="en">
        <ApiRequestBar
          onNameCommit={onNameCommit}
          onSave={onSave}
          onSend={vi.fn()}
          onUpdate={vi.fn()}
          tab={requestTab()}
        />
      </I18nProvider>,
    );

    fireEvent.click(screen.getByRole("button", { name: "Edit name" }));
    const input = screen.getByRole("textbox", { name: "Request name" });
    fireEvent.change(input, { target: { value: "Cancelled" } });
    fireEvent.keyDown(input, { key: "Escape" });
    expect(onNameCommit).not.toHaveBeenCalled();

    rerender(
      <I18nProvider initialLocale="en">
        <ApiRequestBar
          onNameCommit={onNameCommit}
          onSave={onSave}
          onSend={vi.fn()}
          onUpdate={vi.fn()}
          tab={requestTab()}
        />
      </I18nProvider>,
    );
    fireEvent.click(screen.getByRole("button", { name: "Edit name" }));
    const blurInput = screen.getByRole("textbox", { name: "Request name" });
    fireEvent.change(blurInput, { target: { value: "Blurred name" } });
    fireEvent.blur(blurInput);

    expect(onNameCommit).toHaveBeenCalledWith("Blurred name");
    expect(onSave).not.toHaveBeenCalled();
  });

  it("passes the current inline name to Ctrl+S before the input blurs", () => {
    const onSave = vi.fn();
    render(
      <I18nProvider initialLocale="en">
        <ApiRequestBar
          onNameCommit={vi.fn()}
          onSave={onSave}
          onSend={vi.fn()}
          onUpdate={vi.fn()}
          tab={requestTab()}
        />
      </I18nProvider>,
    );

    fireEvent.click(screen.getByRole("button", { name: "Edit name" }));
    const input = screen.getByRole("textbox", { name: "Request name" });
    fireEvent.change(input, { target: { value: "List users" } });
    fireEvent.keyDown(input, { key: "s", ctrlKey: true });

    expect(onSave).toHaveBeenCalledTimes(1);
    expect(onSave).toHaveBeenCalledWith("List users");
  });

  it("passes the current inline name when the save button is clicked", () => {
    const onSave = vi.fn();
    render(
      <I18nProvider initialLocale="en">
        <ApiRequestBar
          onNameCommit={vi.fn()}
          onSave={onSave}
          onSend={vi.fn()}
          onUpdate={vi.fn()}
          tab={requestTab()}
        />
      </I18nProvider>,
    );

    fireEvent.click(screen.getByRole("button", { name: "Edit name" }));
    const input = screen.getByRole("textbox", { name: "Request name" });
    fireEvent.change(input, { target: { value: "List users" } });
    const saveButton = screen.getByRole("button", { name: "Save" });
    fireEvent.blur(input, { relatedTarget: saveButton });
    fireEvent.click(saveButton);

    expect(onSave).toHaveBeenCalledWith("List users");
  });
});
