// @vitest-environment jsdom
import { afterEach, describe, expect, it, vi } from "vitest";
import { cleanup, fireEvent, render, screen } from "@testing-library/react";
import type { SftpFileEntry } from "@unfour/command-client";
import { SftpFileList, type SftpFileListActions } from "./SftpFileList";

function entry(
  overrides: Partial<SftpFileEntry> & Pick<SftpFileEntry, "name" | "path" | "kind">,
): SftpFileEntry {
  return {
    size: 12,
    modifiedAt: "2026-01-01T00:00:00.000Z",
    ...overrides,
  };
}

function actions(overrides: Partial<SftpFileListActions> = {}): SftpFileListActions {
  return {
    canGoParent: true,
    canRefresh: true,
    canUpload: true,
    onCopyPath: vi.fn(),
    onDelete: vi.fn(),
    onDownload: vi.fn(),
    onNewFolder: vi.fn(),
    onOpen: vi.fn(),
    onParent: vi.fn(),
    onRefresh: vi.fn(),
    onRename: vi.fn(),
    onUpload: vi.fn(),
    onUploadHere: vi.fn(),
    ...overrides,
  };
}

afterEach(cleanup);

describe("SftpFileList context menu", () => {
  it("shows file actions and keeps delete confirmation as a caller concern", () => {
    const listActions = actions();
    const file = entry({
      kind: "file",
      name: "readme.txt",
      path: "/home/demo/readme.txt",
    });

    render(
      <SftpFileList
        actions={listActions}
        entries={[file]}
        error={null}
        loading={false}
        onActivate={vi.fn()}
        onRetry={vi.fn()}
        onSelect={vi.fn()}
        onSelectRange={vi.fn()}
        onToggleSelect={vi.fn()}
        selectedPaths={[file.path]}
      />,
    );

    fireEvent.contextMenu(screen.getByRole("row", { name: /readme\.txt/i }), {
      clientX: 40,
      clientY: 40,
    });

    expect(screen.getByRole("menuitem", { name: "Download" })).toBeEnabled();
    expect(screen.getByRole("menuitem", { name: "Rename" })).toBeEnabled();
    expect(screen.getByRole("menuitem", { name: "Copy path" })).toBeEnabled();
    fireEvent.click(screen.getByRole("menuitem", { name: "Delete" }));
    expect(listActions.onDelete).toHaveBeenCalledWith(file);
  });

  it("shows directory actions including open and upload here", () => {
    const listActions = actions();
    const folder = entry({
      kind: "directory",
      name: "src",
      path: "/home/demo/src",
      size: 0,
    });

    render(
      <SftpFileList
        actions={listActions}
        entries={[folder]}
        error={null}
        loading={false}
        onActivate={vi.fn()}
        onRetry={vi.fn()}
        onSelect={vi.fn()}
        onSelectRange={vi.fn()}
        onToggleSelect={vi.fn()}
        selectedPaths={[folder.path]}
      />,
    );

    fireEvent.contextMenu(screen.getByRole("row", { name: /src/i }), {
      clientX: 40,
      clientY: 40,
    });

    expect(screen.getByRole("menuitem", { name: "Open" })).toBeInTheDocument();
    expect(screen.getByRole("menuitem", { name: "Upload here" })).toBeInTheDocument();
    expect(screen.queryByRole("menuitem", { name: "Download" })).toBeNull();
  });

  it("shows blank-area actions without rename or delete", () => {
    const listActions = actions({ canGoParent: false, canRefresh: false, canUpload: false });

    render(
      <SftpFileList
        actions={listActions}
        entries={[]}
        error={null}
        loading={false}
        onActivate={vi.fn()}
        onRetry={vi.fn()}
        onSelect={vi.fn()}
        onSelectRange={vi.fn()}
        onToggleSelect={vi.fn()}
        selectedPaths={[]}
      />,
    );

    fireEvent.contextMenu(screen.getByText("This remote directory is empty."), {
      clientX: 24,
      clientY: 24,
    });

    expect(screen.getByRole("menuitem", { name: "Upload" })).toHaveAttribute(
      "aria-disabled",
      "true",
    );
    expect(screen.getByRole("menuitem", { name: "New folder" })).toHaveAttribute(
      "aria-disabled",
      "true",
    );
    expect(screen.getByRole("menuitem", { name: "Refresh" })).toHaveAttribute(
      "aria-disabled",
      "true",
    );
    expect(screen.getByRole("menuitem", { name: "Go to parent" })).toHaveAttribute(
      "aria-disabled",
      "true",
    );
    expect(screen.queryByRole("menuitem", { name: "Rename" })).toBeNull();
    expect(screen.queryByRole("menuitem", { name: "Delete" })).toBeNull();
  });

  it("opens the row menu with Shift+F10", () => {
    const file = entry({
      kind: "file",
      name: "a.txt",
      path: "/a.txt",
    });

    render(
      <SftpFileList
        actions={actions()}
        entries={[file]}
        error={null}
        loading={false}
        onActivate={vi.fn()}
        onRetry={vi.fn()}
        onSelect={vi.fn()}
        onSelectRange={vi.fn()}
        onToggleSelect={vi.fn()}
        selectedPaths={[file.path]}
      />,
    );

    const row = screen.getByRole("row", { name: /a\.txt/i });
    row.focus();
    fireEvent.keyDown(row, { key: "F10", shiftKey: true });

    expect(screen.getByRole("menuitem", { name: "Download" })).toBeInTheDocument();
    expect(screen.getByRole("menuitem", { name: "Copy path" })).toBeInTheDocument();
  });

  it("toggles multi-select with ctrl/meta click", () => {
    const onToggleSelect = vi.fn();
    const first = entry({ kind: "file", name: "a.txt", path: "/a.txt" });
    const second = entry({ kind: "file", name: "b.txt", path: "/b.txt" });

    render(
      <SftpFileList
        actions={actions()}
        entries={[first, second]}
        error={null}
        loading={false}
        onActivate={vi.fn()}
        onRetry={vi.fn()}
        onSelect={vi.fn()}
        onSelectRange={vi.fn()}
        onToggleSelect={onToggleSelect}
        selectedPaths={[first.path]}
      />,
    );

    fireEvent.click(screen.getByRole("row", { name: /b\.txt/i }), { ctrlKey: true });
    expect(onToggleSelect).toHaveBeenCalledWith(second);
  });
});
