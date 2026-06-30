// @vitest-environment jsdom
import { afterEach, describe, expect, it, vi } from "vitest";
import { cleanup, fireEvent, render, screen } from "@testing-library/react";
import { TreeView } from "./tree-view";

afterEach(cleanup);

function dataTransfer(): DataTransfer {
  const data = new Map<string, string>();
  return {
    clearData: vi.fn((format?: string) => {
      if (format) {
        data.delete(format);
      } else {
        data.clear();
      }
    }),
    dropEffect: "move",
    effectAllowed: "all",
    files: [] as unknown as FileList,
    getData: vi.fn((format: string) => data.get(format) ?? ""),
    items: [] as unknown as DataTransferItemList,
    setData: vi.fn((format: string, value: string) => data.set(format, value)),
    types: [],
    setDragImage: vi.fn(),
  };
}

describe("TreeView", () => {
  it("uses a compact disclosure control in sidebar rows", () => {
    render(
      <TreeView
        items={[
          {
            id: "parent",
            label: "Parent",
            children: [{ id: "child", label: "Child" }],
          },
        ]}
      />,
    );

    expect(screen.getByRole("button", { name: "Expand" })).toHaveClass("w-4");
  });

  it("auto-expands content that appears after mount (lazy schema load)", () => {
    const placeholder = (
      <TreeView
        defaultExpandedIds={["db"]}
        items={[
          {
            id: "db",
            label: "appdb",
            children: [{ id: "db:loading", label: "Loading…", disabled: true }],
          },
        ]}
      />
    );
    const { rerender } = render(placeholder);
    expect(screen.queryByText("users")).not.toBeInTheDocument();

    // Schema finished loading: the database now exposes a table group whose id
    // is newly added to defaultExpandedIds. It must expand into view without a
    // remount, mirroring the database connection tree's lazy loading.
    rerender(
      <TreeView
        defaultExpandedIds={["db", "db:tables"]}
        items={[
          {
            id: "db",
            label: "appdb",
            children: [
              {
                id: "db:tables",
                label: "Tables",
                children: [{ id: "db:tables:users", label: "users" }],
              },
            ],
          },
        ]}
      />,
    );

    expect(screen.getByText("users")).toBeInTheDocument();
  });

  it("fires onToggle when a node auto-expands so lazy children can load", () => {
    const onToggle = vi.fn();
    const items = [{ id: "conn", label: "Connection", children: [{ id: "conn:hint", label: "Expand to load", disabled: true }] }];
    const { rerender } = render(<TreeView items={items} onToggle={onToggle} />);
    expect(onToggle).not.toHaveBeenCalled();

    // The connection becomes the auto-expand target (e.g. after connecting). It
    // must notify onToggle exactly as a manual expand would, so its database
    // list is fetched without the user clicking the disclosure.
    rerender(<TreeView defaultExpandedIds={["conn"]} items={items} onToggle={onToggle} />);

    expect(onToggle).toHaveBeenCalledWith("conn", true);
  });

  it("reports the correct expanded state when the disclosure is clicked", () => {
    const onToggle = vi.fn();
    render(
      <TreeView
        items={[{ id: "db", label: "appdb", children: [{ id: "db:hint", label: "Expand to load", disabled: true }] }]}
        onToggle={onToggle}
      />,
    );

    // Manually expanding a collapsed node must report expanded=true so consumers
    // run their lazy load. Regression: this previously reported false because the
    // flag was read before React invoked the state updater.
    fireEvent.click(screen.getByRole("button", { name: "Expand" }));
    expect(onToggle).toHaveBeenLastCalledWith("db", true);

    fireEvent.click(screen.getByRole("button", { name: "Collapse" }));
    expect(onToggle).toHaveBeenLastCalledWith("db", false);
  });

  it("calls onDrop when a draggable item is dropped on an accepted target", () => {
    const onDrop = vi.fn();
    render(
      <TreeView
        canDrag={(item) => item.id === "request"}
        canDrop={(source, target) => source.id === "request" && target.id === "folder"}
        defaultExpandedIds={["folder"]}
        items={[
          {
            id: "folder",
            label: "Folder",
            children: [{ id: "request", label: "Request" }],
          },
        ]}
        onDrop={onDrop}
      />,
    );

    const requestRow = screen.getByText("Request").closest("[role='treeitem']");
    const folderRow = screen.getByText("Folder").closest("[role='treeitem']");
    expect(requestRow).not.toBeNull();
    expect(folderRow).not.toBeNull();

    const transfer = dataTransfer();
    fireEvent.dragStart(requestRow as HTMLElement, { dataTransfer: transfer });
    fireEvent.dragOver(folderRow as HTMLElement, { dataTransfer: transfer });
    fireEvent.drop(folderRow as HTMLElement, { dataTransfer: transfer });

    expect(onDrop).toHaveBeenCalledWith(expect.objectContaining({
      position: "inside",
      source: expect.objectContaining({ id: "request" }),
      target: expect.objectContaining({ id: "folder" }),
    }));
  });

  it("keeps internal tree dragging out of native HTML drag mode", () => {
    render(
      <TreeView
        canDrag={(item) => item.id === "request"}
        items={[{ id: "request", label: "Request" }]}
        onDrop={vi.fn()}
      />,
    );

    const requestLabel = screen.getByRole("button", { name: "Request" });
    expect(requestLabel).not.toHaveAttribute("draggable", "true");
    expect(requestLabel.closest("[role='treeitem']")).not.toHaveAttribute(
      "draggable",
      "true",
    );
  });

  it("uses the dataTransfer item id when drop target handlers run before drag state renders", () => {
    const onDrop = vi.fn();
    render(
      <TreeView
        canDrag={(item) => item.id === "request"}
        canDrop={(source, target) => source.id === "request" && target.id === "folder"}
        defaultExpandedIds={["folder"]}
        items={[
          {
            id: "folder",
            label: "Folder",
            children: [{ id: "request", label: "Request" }],
          },
        ]}
        onDrop={onDrop}
      />,
    );

    const folderRow = screen.getByText("Folder").closest("[role='treeitem']");
    expect(folderRow).not.toBeNull();

    const transfer = dataTransfer();
    transfer.setData("text/plain", "request");
    fireEvent.dragOver(folderRow as HTMLElement, { dataTransfer: transfer });
    fireEvent.drop(folderRow as HTMLElement, { dataTransfer: transfer });

    expect(onDrop).toHaveBeenCalledWith(expect.objectContaining({
      position: "inside",
      source: expect.objectContaining({ id: "request" }),
      target: expect.objectContaining({ id: "folder" }),
    }));
  });

  it("marks before drop positions for sortable rows", () => {
    const onDrop = vi.fn();
    render(
      <TreeView
        canDrag={(item) => item.id === "first"}
        canDrop={(source, target, position) =>
          source.id === "first" && target.id === "second" && position === "before"
        }
        items={[
          { id: "first", label: "First" },
          { id: "second", label: "Second" },
        ]}
        onDrop={onDrop}
      />,
    );

    const firstRow = screen.getByText("First").closest("[role='treeitem']");
    const secondRow = screen.getByText("Second").closest("[role='treeitem']");
    expect(firstRow).not.toBeNull();
    expect(secondRow).not.toBeNull();
    vi.spyOn(secondRow as HTMLElement, "getBoundingClientRect").mockReturnValue({
      bottom: 34,
      height: 24,
      left: 0,
      right: 120,
      top: 10,
      width: 120,
      x: 0,
      y: 10,
      toJSON: () => ({}),
    });

    const transfer = dataTransfer();
    fireEvent.dragStart(firstRow as HTMLElement, { dataTransfer: transfer });
    fireEvent.dragOver(secondRow as HTMLElement, {
      clientY: 11,
      dataTransfer: transfer,
    });

    expect(secondRow).toHaveAttribute("data-drop-position", "before");

    fireEvent.drop(secondRow as HTMLElement, {
      clientY: 11,
      dataTransfer: transfer,
    });

    expect(onDrop).toHaveBeenCalledWith(expect.objectContaining({
      position: "before",
      source: expect.objectContaining({ id: "first" }),
      target: expect.objectContaining({ id: "second" }),
    }));
  });

  it("drops with pointer dragging when native drag events are unavailable", () => {
    const onDrop = vi.fn();
    const onSelect = vi.fn();
    render(
      <TreeView
        canDrag={(item) => item.id === "request"}
        canDrop={(source, target) => source.id === "request" && target.id === "folder"}
        defaultExpandedIds={["folder"]}
        items={[
          {
            id: "folder",
            label: "Folder",
            children: [{ id: "request", label: "Request" }],
          },
        ]}
        onDrop={onDrop}
        onSelect={onSelect}
      />,
    );

    const requestLabel = screen.getByRole("button", { name: "Request" });
    const folderRow = screen.getByText("Folder").closest("[role='treeitem']");
    expect(folderRow).not.toBeNull();
    const originalElementFromPoint = document.elementFromPoint;
    const elementFromPoint = vi.fn(() => folderRow as Element);
    Object.defineProperty(document, "elementFromPoint", {
      configurable: true,
      value: elementFromPoint,
    });

    try {
      fireEvent.pointerDown(requestLabel, {
        button: 0,
        clientX: 12,
        clientY: 12,
        pointerId: 1,
      });
      fireEvent.pointerMove(requestLabel, {
        clientX: 28,
        clientY: 28,
        pointerId: 1,
      });
      fireEvent.pointerUp(requestLabel, {
        clientX: 28,
        clientY: 28,
        pointerId: 1,
      });
      fireEvent.click(requestLabel);

      expect(onDrop).toHaveBeenCalledWith(expect.objectContaining({
        position: "inside",
        source: expect.objectContaining({ id: "request" }),
        target: expect.objectContaining({ id: "folder" }),
      }));
      expect(onSelect).not.toHaveBeenCalled();
    } finally {
      Object.defineProperty(document, "elementFromPoint", {
        configurable: true,
        value: originalElementFromPoint,
      });
    }
  });
});
