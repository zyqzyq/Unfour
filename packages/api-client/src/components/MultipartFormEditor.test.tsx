// @vitest-environment jsdom
import { useState } from "react";
import { cleanup, fireEvent, render, screen, waitFor } from "@testing-library/react";
import { afterEach, describe, expect, it, vi } from "vitest";
import { I18nProvider } from "@unfour/ui";
import { pickApiRequestFile } from "@unfour/command-client";
import { MultipartFormEditor } from "./MultipartFormEditor";
import type { MultipartPart } from "../model/types";

vi.mock("@unfour/command-client", () => ({ pickApiRequestFile: vi.fn() }));
afterEach(() => { cleanup(); vi.resetAllMocks(); });
const file: MultipartPart = { id: "f", enabled: true, key: "avatar", type: "file", fileName: "avatar.png", filePath: null };
function Editor() {
  const [parts, setParts] = useState<MultipartPart[]>([file]);
  return <I18nProvider initialLocale="en"><MultipartFormEditor parts={parts} onChange={setParts} /></I18nProvider>;
}
describe("multipart file editor", () => {
  it("shows saved filename as unselected, selects and clears without displaying the path", async () => {
    vi.mocked(pickApiRequestFile).mockResolvedValue({ name: "avatar.png", path: "C:/private/avatar.png" });
    const { container } = render(<Editor />);
    expect(screen.getByText(/file not selected/)).toBeTruthy();
    fireEvent.click(screen.getByRole("button", { name: "Select File" }));
    await waitFor(() => expect(screen.queryByText(/file not selected/)).toBeNull());
    expect(container.innerHTML).not.toContain("C:/private");
    fireEvent.click(screen.getByRole("button", { name: "Clear" }));
    expect(screen.getByText(/file not selected/)).toBeTruthy();
    expect(screen.queryByText("avatar.png")).toBeNull();
  });
  it("keeps definition on cancel and sanitizes picker failures", async () => {
    vi.mocked(pickApiRequestFile).mockResolvedValueOnce(null).mockRejectedValueOnce(new Error("C:/private/path"));
    const { container } = render(<Editor />);
    fireEvent.click(screen.getByRole("button", { name: "Select File" }));
    await waitFor(() => expect(screen.getByRole("button", { name: "Select File" }).hasAttribute("disabled")).toBe(false));
    expect(screen.getByText("avatar.png")).toBeTruthy();
    fireEvent.click(screen.getByRole("button", { name: "Select File" }));
    await screen.findByRole("alert");
    expect(container.innerHTML).not.toContain("C:/private");
  });
  it("supports row type, enabled, add and delete controls", () => {
    render(<Editor />);
    fireEvent.change(screen.getByRole("combobox", { name: "Type" }), { target: { value: "text" } });
    expect(screen.getByRole("textbox", { name: "Value" })).toBeTruthy();
    fireEvent.click(screen.getByRole("checkbox", { name: "Enabled" }));
    expect((screen.getByRole("checkbox", { name: "Enabled" }) as HTMLInputElement).checked).toBe(false);
    fireEvent.click(screen.getByRole("button", { name: "Add row" }));
    expect(screen.getAllByRole("combobox", { name: "Type" })).toHaveLength(2);
    fireEvent.click(screen.getAllByRole("button", { name: "Delete row" })[0]);
    expect(screen.getAllByRole("combobox", { name: "Type" })).toHaveLength(1);
  });
});
