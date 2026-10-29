// @vitest-environment jsdom
import { cleanup, fireEvent, render, screen, waitFor } from "@testing-library/react";
import { afterEach, expect, it, vi } from "vitest";
import { Tabs } from "./tabs";
import { I18nProvider } from "./i18n";

afterEach(cleanup);

it.each([
  ["en", "Close A", "Unsaved changes", "Cancel", "Discard"],
  ["zh-CN", "关闭 A", "未保存的更改", "取消", "放弃更改"],
] as const)("uses an accessible localized confirmation (%s)", async (locale, close, title, cancel, discard) => {
  const onClose = vi.fn();
  render(<I18nProvider initialLocale={locale}><Tabs activeId="a" onClose={onClose} onSelect={vi.fn()} tabs={[{ id: "a", title: "A", modified: true }]} /></I18nProvider>);
  const trigger = screen.getByRole("button", { name: close });
  trigger.focus();
  fireEvent.click(trigger);
  const dialog = screen.getByRole("dialog", { name: title });
  expect(screen.getByRole("button", { name: cancel })).toHaveFocus();
  fireEvent.keyDown(dialog, { key: "Escape" });
  await waitFor(() => expect(screen.queryByRole("dialog")).not.toBeInTheDocument());
  expect(onClose).not.toHaveBeenCalled();
  expect(trigger).toHaveFocus();
  fireEvent.click(trigger);
  fireEvent.click(screen.getByRole("button", { name: discard }));
  expect(onClose).toHaveBeenCalledExactlyOnceWith("a");
});

it.each(["Close Others", "Close to the Right"])("does not discard modified tabs via %s", async (action) => {
  const onClose = vi.fn();
  render(<Tabs activeId="a" onClose={onClose} onSelect={vi.fn()} tabs={[{ id: "a", title: "A" }, { id: "b", title: "B", modified: true }, { id: "c", title: "C", modified: true }]} />);
  fireEvent.contextMenu(screen.getByRole("tab", { name: "A" }));
  expect(screen.queryByText("Ctrl+W")).not.toBeInTheDocument();
  fireEvent.click(screen.getByRole("menuitem", { name: action }));
  await screen.findByRole("dialog");
  expect(onClose).not.toHaveBeenCalled();
  fireEvent.click(screen.getByRole("button", { name: "Cancel" }));
  expect(onClose).not.toHaveBeenCalled();
  fireEvent.contextMenu(screen.getByRole("tab", { name: "A" }));
  fireEvent.click(screen.getByRole("menuitem", { name: action }));
  fireEvent.click(await screen.findByRole("button", { name: "Discard" }));
  expect(onClose.mock.calls).toEqual([["b"], ["c"]]);
});

it("routes a custom-menu close through the same modified confirmation", async () => {
  const onAction = vi.fn();
  render(<Tabs activeId="a" onAction={onAction} onSelect={vi.fn()} tabs={[{ id: "a", title: "A", modified: true, contextMenu: <span>Custom</span> }]} />);
  fireEvent.contextMenu(screen.getByRole("tab", { name: "A" }));
  fireEvent.click(screen.getByRole("menuitem", { name: "Close" }));
  await screen.findByRole("dialog");
  expect(onAction).not.toHaveBeenCalled();
  fireEvent.click(screen.getByRole("button", { name: "Discard" }));
  expect(onAction).toHaveBeenCalledExactlyOnceWith({ close: "a" });
});

it.each(["Close Others", "Close to the Right"])("dispatches %s to the owner once after confirming", async (label) => {
  const onAction = vi.fn();
  render(<Tabs activeId="a" onAction={onAction} onSelect={vi.fn()} tabs={[{ id: "a", title: "A" }, { id: "b", title: "B", modified: true }, { id: "c", title: "C", modified: true }]} />);
  fireEvent.contextMenu(screen.getByRole("tab", { name: "A" }));
  fireEvent.click(screen.getByRole("menuitem", { name: label }));
  await screen.findByRole("dialog");
  expect(onAction).not.toHaveBeenCalled();
  fireEvent.click(screen.getByRole("button", { name: "Discard" }));
  expect(onAction).toHaveBeenCalledExactlyOnceWith(label === "Close Others" ? { closeOthers: "a", keep: "a" } : { closeRight: "a", keep: "a" });
});

it("renders drag state immediately and reorders exactly once, clearing it on drop/end", () => {
  const reorder = vi.fn();
  render(<Tabs activeId="a" onReorder={reorder} onSelect={vi.fn()} tabs={[{ id: "a", title: "A" }, { id: "b", title: "B" }]} />);
  const first = screen.getByRole("tab", { name: "A" }).closest("[draggable]")!;
  const second = screen.getByRole("tab", { name: "B" }).closest("[draggable]")!;
  fireEvent.dragStart(first, { dataTransfer: { setData: vi.fn() } });
  expect(first).toHaveClass("opacity-40");
  fireEvent.dragOver(second, { dataTransfer: {} });
  fireEvent.drop(second);
  expect(reorder).toHaveBeenCalledExactlyOnceWith(0, 1);
  expect(first).not.toHaveClass("opacity-40");
  fireEvent.dragEnd(first);
  expect(reorder).toHaveBeenCalledTimes(1);
});
