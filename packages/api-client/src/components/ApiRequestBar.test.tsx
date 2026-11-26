// @vitest-environment jsdom
import { emptyDraft, type ApiRequestTab } from "../model/request-tabs";
import { cleanup, fireEvent, render, screen } from "@testing-library/react";
import { I18nProvider } from "@unfour/ui";
import { afterEach, describe, expect, it, vi } from "vitest";
import { ApiRequestBar } from "./ApiRequestBar";

function requestTab(): ApiRequestTab {
  return {
    baseline: null,
    cancelling: false,
    draft: {
      auth: { type: "none" },
      body: "",
      bodyMode: "none",
      collectionId: null,
      envVariables: [],
      preRequestScript: "",
      postResponseScript: "",
      multipartParts: [],
      formBody: [],
      headers: [],
      method: "GET",
      name: "",
      parentFolderId: null,
      query: [],
      rawBodyType: "json",
      timeoutMs: null,
      url: "https://api.example.com/resource",
    },
    id: "new:1",
    execution: null,
    executionId: null,
    lastRequest: null,
    requestTab: "query",
    response: null,
    responseTab: "body",
    saveError: null,
    savedRequestId: null,
    saving: false,
    sendError: null,
    sendErrorCode: null,
    sending: false,
    source: "new",
    sourceId: null,
  };
}

afterEach(() => {
  cleanup();
});

describe("ApiRequestBar", () => {
  it("discards a pending cURL preview if its destination draft changes", () => {
    const tab = { ...requestTab(), draft: emptyDraft() };
    const onSave = vi.fn();
    const onUpdate = vi.fn();
    const view = (current: ApiRequestTab) => <I18nProvider initialLocale="en">
      <ApiRequestBar tab={current} onNameCommit={vi.fn()} onSave={onSave} onSend={vi.fn()} onUpdate={onUpdate} />
    </I18nProvider>;
    const { rerender } = render(view(tab));
    fireEvent.paste(screen.getByLabelText("Request URL"), { clipboardData: { getData: () => "curl https://example.test" } });
    fireEvent.keyDown(screen.getByRole("button", { name: "Apply to this request" }), { key: "s", ctrlKey: true });
    expect(onSave).not.toHaveBeenCalled();
    rerender(view({ ...tab, draft: { ...tab.draft, name: "Edited elsewhere" } }));
    expect(screen.queryByRole("dialog")).toBeNull();
    rerender(view(tab));
    expect(screen.queryByRole("dialog")).toBeNull();
    expect(onUpdate).not.toHaveBeenCalled();
  });

  it("leaves native paste available for existing requests and unsaved edits outside the URL field", () => {
    const blank = { ...requestTab(), draft: emptyDraft() };
    const onUpdate = vi.fn();
    const view = (tab: ApiRequestTab) => <I18nProvider initialLocale="en">
      <ApiRequestBar tab={tab} onNameCommit={vi.fn()} onSave={vi.fn()} onSend={vi.fn()} onUpdate={onUpdate} />
    </I18nProvider>;
    const { rerender } = render(view(blank));
    const clipboardData = { getData: () => "curl https://example.test" };
    for (const tab of [
      { ...blank, source: "saved" as const },
      { ...blank, source: "history" as const },
      { ...blank, draft: { ...blank.draft, headers: [{ key: "Version", value: "1", enabled: true }] } },
      { ...blank, draft: { ...blank.draft, temporaryHandle: "session" } },
    ]) {
      rerender(view(tab));
      expect(fireEvent.paste(screen.getByLabelText("Request URL"), { clipboardData })).toBe(true);
      expect(screen.queryByRole("dialog")).toBeNull();
    }
    expect(onUpdate).not.toHaveBeenCalled();
  });

  it("does not save while the name input is composing or has consumed the shortcut", () => {
    const onSave = vi.fn();
    render(<I18nProvider initialLocale="en"><ApiRequestBar tab={requestTab()} onNameCommit={vi.fn()} onSave={onSave} onSend={vi.fn()} onUpdate={vi.fn()} /></I18nProvider>);
    fireEvent.click(screen.getByRole("button", { name: "Edit name" }));
    const input = screen.getByRole("textbox", { name: "Request name" });
    fireEvent.keyDown(input, { key: "s", ctrlKey: true, isComposing: true });
    fireEvent.keyDown(input, { key: "s", metaKey: true, keyCode: 229 });
    input.addEventListener("keydown", (event) => event.preventDefault());
    fireEvent.keyDown(input, { key: "s", ctrlKey: true });
    expect(onSave).not.toHaveBeenCalled();
  });

  it("labels history as a lossy snapshot and requires explicit review for a new send", () => {
    const tab = { ...requestTab(), source: "history" as const };
    const onUpdate = vi.fn();
    render(<I18nProvider initialLocale="en"><ApiRequestBar tab={tab} onNameCommit={vi.fn()} onSave={vi.fn()} onSend={vi.fn()} onUpdate={onUpdate} /></I18nProvider>);
    expect(screen.getByText(/Auth, scripts and variable references cannot be restored/)).toBeTruthy();
    expect(screen.getByRole("button", { name: "Send New Request" })).toBeDisabled();
    fireEvent.click(screen.getByRole("checkbox"));
    expect(onUpdate).toHaveBeenCalledWith({ historyReviewConfirmed: true });
  });
  it("replaces Send with Stop and prevents repeated cancellation", () => {
    const onStop = vi.fn();
    const sending = { ...requestTab(), sending: true };
    const { rerender } = render(
      <I18nProvider initialLocale="en">
        <ApiRequestBar
          onNameCommit={vi.fn()}
          onSave={vi.fn()}
          onSend={vi.fn()}
          onStop={onStop}
          onUpdate={vi.fn()}
          tab={sending}
        />
      </I18nProvider>,
    );

    fireEvent.click(screen.getByRole("button", { name: "Stop" }));
    expect(onStop).toHaveBeenCalledTimes(1);
    expect(screen.queryByRole("button", { name: "Send" })).toBeNull();

    rerender(
      <I18nProvider initialLocale="en">
        <ApiRequestBar
          onNameCommit={vi.fn()}
          onSave={vi.fn()}
          onSend={vi.fn()}
          onStop={onStop}
          onUpdate={vi.fn()}
          tab={{ ...sending, cancelling: true }}
        />
      </I18nProvider>,
    );
    expect(screen.getByRole("button", { name: "Stopping" })).toBeDisabled();
  });

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
