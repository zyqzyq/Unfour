// @vitest-environment jsdom
import { cleanup, fireEvent, render, screen } from "@testing-library/react";
import { I18nProvider } from "@unfour/ui";
import { afterEach, describe, expect, it, vi } from "vitest";
import { ApiSaveDialog } from "./ApiSaveDialog";

afterEach(cleanup);

function renderDialog(overrides: Partial<Parameters<typeof ApiSaveDialog>[0]> = {}) {
  const onSave = vi.fn();
  const onCancel = vi.fn();
  render(
    <I18nProvider initialLocale="en">
      <ApiSaveDialog
        collections={[]}
        defaultCollectionId={null}
        defaultName=""
        defaultParentFolderId={null}
        folders={[]}
        onCancel={onCancel}
        onSave={onSave}
        open
        savedRequests={[]}
        saving={false}
        {...overrides}
      />
    </I18nProvider>,
  );
  return { onCancel, onSave };
}

describe("ApiSaveDialog", () => {
  it("submits the entered name with Enter", () => {
    const { onSave } = renderDialog({ defaultName: "Health check" });

    fireEvent.submit(screen.getByRole("button", { name: "Save" }).closest("form")!);

    expect(onSave).toHaveBeenCalledTimes(1);
    expect(onSave).toHaveBeenCalledWith({
      collectionId: null,
      name: "Health check",
      parentFolderId: null,
    });
  });

  it("keeps existing disabled and duplicate validation on submit", () => {
    const { onSave } = renderDialog({ defaultName: "   " });
    fireEvent.submit(screen.getByRole("button", { name: "Save" }).closest("form")!);
    expect(onSave).not.toHaveBeenCalled();
    expect(screen.getByRole("button", { name: "Save" })).toBeDisabled();
  });

  it("shows a save failure inside the dialog and keeps the entered name", () => {
    renderDialog({
      defaultName: "Create user",
      error: "Workspace storage is full",
    });

    expect(screen.getByRole("dialog")).toHaveTextContent("Workspace storage is full");
    expect(screen.getByDisplayValue("Create user")).toBeInTheDocument();
    expect(screen.getByRole("button", { name: "Save" })).toBeEnabled();
  });
});
