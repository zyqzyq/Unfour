// @vitest-environment jsdom
import { cleanup, fireEvent, render, screen } from "@testing-library/react";
import { I18nProvider } from "@unfour/ui";
import { afterEach, expect, it, vi } from "vitest";
import { ApiCurlImport } from "./ApiCurlImport";
import { resetApiRequestTabStore, useApiRequestTabStore } from "../model/api-request-tab-state";

afterEach(() => { cleanup(); resetApiRequestTabStore(); });
it("supports the Chinese flow and returns focus after cancellation", async () => {
  const imported = vi.fn();
  render(<I18nProvider initialLocale="zh-CN"><ApiCurlImport onImport={imported} /></I18nProvider>);
  const trigger = screen.getByRole("button", { name: "导入 cURL" });
  fireEvent.click(trigger);
  fireEvent.change(screen.getByLabelText("cURL 命令"), { target: { value: "curl https://example.test" } });
  fireEvent.click(screen.getByRole("button", { name: "预览" }));
  expect(screen.getByLabelText("HTTP 方法")).toHaveValue("GET");
  fireEvent.click(screen.getByRole("button", { name: "取消" }));
  expect(imported).not.toHaveBeenCalled();
  expect(screen.queryByRole("dialog")).not.toBeInTheDocument();
  await vi.waitFor(() => expect(trigger).toHaveFocus());
});
it("keeps failed input, invalidates stale preview, masks secrets and creates only an unsaved tab", () => {
  const imported = vi.fn(draft => useApiRequestTabStore.getState().newRequest("ws", draft));
  render(<I18nProvider><ApiCurlImport onImport={imported} /></I18nProvider>);
  fireEvent.click(screen.getByRole("button", { name: "Import cURL" }));
  const source = screen.getByLabelText("cURL command");
  fireEvent.change(source, { target: { value: "curl https://example.test | whoami" } });
  fireEvent.click(screen.getByRole("button", { name: "Preview" }));
  expect(source).toHaveValue("curl https://example.test | whoami");
  expect(screen.getByRole("alert")).not.toHaveTextContent("whoami");
  expect(imported).not.toHaveBeenCalled();
  fireEvent.change(source, { target: { value: "curl https://example.test -H 'Authorization: Bearer fixture' -H 'Cookie: fixture'" } });
  fireEvent.click(screen.getByRole("button", { name: "Preview" }));
  expect(screen.getByLabelText("Authorization")).toHaveAttribute("type", "password");
  expect(screen.getByLabelText("Cookie")).toHaveAttribute("type", "password");
  expect(imported).not.toHaveBeenCalled();
  fireEvent.change(source, { target: { value: "curl https://example.test -d 'a=1'" } });
  expect(screen.getByRole("button", { name: "Create request" })).toBeDisabled();
  fireEvent.click(screen.getByRole("button", { name: "Preview" }));
  fireEvent.click(screen.getByRole("button", { name: "Create request" }));
  expect(imported).toHaveBeenCalledTimes(1);
  const state = useApiRequestTabStore.getState().byWorkspace.ws;
  const tab = state.tabs.find(item => item.id === state.activeTabId)!;
  expect(tab).toMatchObject({ source: "new", savedRequestId: null, baseline: null, sending: false, saving: false, execution: null, response: null });
  expect(tab.draft).toMatchObject({ preRequestScript: "", postResponseScript: "", body: "a=1" });
  fireEvent.click(screen.getByRole("button", { name: "Import cURL" }));
  expect(screen.getByLabelText("cURL command")).toHaveValue("");
});
