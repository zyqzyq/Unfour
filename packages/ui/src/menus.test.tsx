// @vitest-environment jsdom
import { afterEach, describe, expect, it, vi } from "vitest";
import { cleanup, fireEvent, render, screen } from "@testing-library/react";
import {
  ContextMenu,
  ContextMenuContent,
  ContextMenuItem,
  ContextMenuTrigger,
  DropdownMenuContent,
  DropdownMenuItem,
} from "./menus";
import { DropdownMenu, DropdownMenuTrigger } from "./menus-primitives";

afterEach(cleanup);

describe("ContextMenu", () => {
  it("moves focus into menu items with keyboard navigation", () => {
    const onSelect = vi.fn();

    render(
      <ContextMenu>
        <ContextMenuTrigger asChild>
          <button type="button">Request tab</button>
        </ContextMenuTrigger>
        <ContextMenuContent>
          <ContextMenuItem onSelect={onSelect}>Open</ContextMenuItem>
          <ContextMenuItem>Duplicate</ContextMenuItem>
        </ContextMenuContent>
      </ContextMenu>,
    );

    fireEvent.contextMenu(screen.getByRole("button", { name: "Request tab" }), {
      clientX: 24,
      clientY: 24,
    });

    const menu = screen.getByRole("menu");
    const openItem = screen.getByRole("menuitem", { name: "Open" });

    fireEvent.keyDown(menu, { key: "ArrowDown" });

    expect(openItem).toHaveFocus();
  });
});

describe("DropdownMenu", () => {
  it("portals outside clipped parents and preserves selection and focus return", async () => {
    const onSelect = vi.fn();
    const { container } = render(
      <div style={{ overflow: "hidden", height: 24 }}>
        <DropdownMenu>
          <DropdownMenuTrigger asChild><button>Actions</button></DropdownMenuTrigger>
          <DropdownMenuContent align="end" className="custom-menu">
            <DropdownMenuItem onSelect={onSelect}>Open</DropdownMenuItem>
          </DropdownMenuContent>
        </DropdownMenu>
      </div>,
    );
    const trigger = screen.getByRole("button", { name: "Actions" });
    fireEvent.keyDown(trigger, { key: "ArrowDown" });
    const menu = await screen.findByRole("menu");
    expect(container).not.toContainElement(menu);
    expect(document.body).toContainElement(menu);
    expect(menu).toBeVisible();
    expect(menu).toHaveClass("custom-menu");
    expect(menu).toHaveAttribute("data-align", "end");
    fireEvent.click(screen.getByRole("menuitem", { name: "Open" }));
    expect(onSelect).toHaveBeenCalledOnce();
    await vi.waitFor(() => expect(trigger).toHaveFocus());
  });

  it("keeps menu item weight normal when opened from a bold tree row", async () => {
    render(
      <div className="font-semibold">
        <DropdownMenu>
          <DropdownMenuTrigger asChild>
            <button type="button">More</button>
          </DropdownMenuTrigger>
          <DropdownMenuContent>
            <DropdownMenuItem>Open</DropdownMenuItem>
          </DropdownMenuContent>
        </DropdownMenu>
      </div>,
    );

    fireEvent.pointerDown(screen.getByRole("button", { name: "More" }));

    expect(await screen.findByRole("menu")).toHaveClass("font-normal");
  });
});
