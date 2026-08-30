// @vitest-environment jsdom
import { cleanup, fireEvent, render, screen } from "@testing-library/react";
import { afterEach, expect, it, vi } from "vitest";
import { Tabs } from "./tabs";

afterEach(cleanup);

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
