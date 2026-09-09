// @vitest-environment jsdom
import { useState } from "react";
import { afterEach, describe, expect, it, vi } from "vitest";
import { cleanup, fireEvent, render, screen, waitFor } from "@testing-library/react";
import { CommandPalette } from "./command-palette";

afterEach(cleanup);

describe("CommandPalette", () => {
  it("filters labels, selects with arrows, and runs the selected command with Enter", () => {
    const onApi = vi.fn();
    const onSsh = vi.fn();
    render(<CommandPalette open onClose={vi.fn()} items={[
      { id: "api", label: "Open API Client", onSelect: onApi },
      { id: "ssh", label: "Open SSH Terminal", onSelect: onSsh },
    ]} />);
    const input = screen.getByRole("combobox");
    expect(input).toHaveFocus();
    fireEvent.keyDown(input, { key: "ArrowDown" });
    expect(screen.getByRole("option", { name: "Open SSH Terminal" })).toHaveAttribute("aria-selected", "true");
    fireEvent.keyDown(input, { key: "Enter" });
    expect(onSsh).toHaveBeenCalledTimes(1);
    fireEvent.change(input, { target: { value: "  API open " } });
    expect(screen.getAllByRole("option")).toHaveLength(1);
    fireEvent.keyDown(input, { key: "Enter" });
    expect(onApi).toHaveBeenCalledTimes(1);
  });

  it("supports rich extension labels and explicit search text", () => {
    render(<CommandPalette open onClose={vi.fn()} items={[
      { id: "sync", label: <span>Sync <strong>workspace</strong></span>, onSelect: vi.fn() },
      { id: "custom", label: <span>Custom</span>, searchText: "Cloud settings", onSelect: vi.fn() },
    ]} />);
    fireEvent.change(screen.getByRole("combobox"), { target: { value: "sync workspace" } });
    expect(screen.getByRole("option", { name: "Sync workspace" })).toBeInTheDocument();
    fireEvent.change(screen.getByRole("combobox"), { target: { value: "cloud" } });
    expect(screen.getByRole("option", { name: "Custom" })).toBeInTheDocument();
  });

  it("does not execute for no results or an IME composition", () => {
    const run = vi.fn();
    render(<CommandPalette open onClose={vi.fn()} items={[{ id: "ssh", label: "Open SSH", onSelect: run }]} />);
    const input = screen.getByRole("combobox");
    fireEvent.keyDown(input, { key: "Enter", isComposing: true });
    fireEvent.change(input, { target: { value: "unknown" } });
    fireEvent.keyDown(input, { key: "ArrowDown" });
    fireEvent.keyDown(input, { key: "Enter" });
    expect(screen.getByRole("status")).toHaveTextContent("No matching commands");
    expect(input).not.toHaveAttribute("aria-activedescendant");
    expect(run).not.toHaveBeenCalled();
  });

  it("closes on Escape, restores focus, and resets the search when reopened", async () => {
    function Harness() {
      const [open, setOpen] = useState(false);
      return <>
        <button onClick={() => setOpen(true)}>Commands</button>
        <CommandPalette open={open} onClose={() => setOpen(false)} items={[{ id: "api", label: "API", onSelect: vi.fn() }]} />
      </>;
    }
    render(<Harness />);
    const trigger = screen.getByRole("button", { name: "Commands" });
    trigger.focus();
    fireEvent.click(trigger);
    fireEvent.change(screen.getByRole("combobox"), { target: { value: "unknown" } });
    fireEvent.keyDown(screen.getByRole("combobox"), { key: "Escape" });
    await waitFor(() => expect(trigger).toHaveFocus());
    expect(screen.queryByRole("dialog")).toBeNull();
    fireEvent.click(trigger);
    expect(screen.getByRole("combobox")).toHaveValue("");
  });
});
