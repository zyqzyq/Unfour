// @vitest-environment jsdom
import { afterEach, describe, expect, it, vi } from "vitest";
import { cleanup, fireEvent, render, screen } from "@testing-library/react";
import { TreeView } from "./tree-view";

afterEach(cleanup);

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
});
