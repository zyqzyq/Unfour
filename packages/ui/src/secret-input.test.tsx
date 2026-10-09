// @vitest-environment jsdom
import { useState } from "react";
import { afterEach, expect, it, vi } from "vitest";
import { cleanup, fireEvent, render, screen, waitFor } from "@testing-library/react";
import { SecretInput, SavedSecretInput } from "./secret-input";

afterEach(cleanup);

it("changes visibility without changing a secret or invoking the editor callback", () => {
  const change = vi.fn();
  render(<SecretInput aria-label="Token" value="{{token}}" onChange={change} />);
  const input = screen.getByLabelText("Token");
  expect(input).toHaveAttribute("type", "password");
  fireEvent.click(screen.getByRole("button", { name: "Show secret value" }));
  expect(input).toHaveAttribute("type", "text");
  fireEvent.click(screen.getByRole("button", { name: "Hide secret value" }));
  expect(input).toHaveValue("{{token}}");
  expect(change).not.toHaveBeenCalled();
});

it("keeps viewing separate from retaining, replacing and clearing a saved value", async () => {
  const change = vi.fn();
  const reveal = vi.fn().mockResolvedValue("saved-secret");
  function Editor() {
    const [value, setValue] = useState("");
    const [cleared, setCleared] = useState(false);
    return <SavedSecretInput saved cleared={cleared} value={value}
      onReveal={reveal} onChange={(v) => { change(v); setValue(v); setCleared(false); }}
      onKeep={() => { setValue(""); setCleared(false); }}
      onClear={() => { setValue(""); setCleared(true); }} />;
  }
  render(<Editor />);
  fireEvent.click(screen.getByRole("button", { name: "Show secret value" }));
  await screen.findByDisplayValue("saved-secret");
  expect(change).not.toHaveBeenCalled();
  fireEvent.click(screen.getByRole("button", { name: "Hide secret value" }));
  expect(screen.queryByDisplayValue("saved-secret")).toBeNull();
  fireEvent.change(screen.getByPlaceholderText("Saved credential; type to replace"), { target: { value: "replacement" } });
  expect(screen.getByText("Replace on save")).toBeInTheDocument();
  fireEvent.click(screen.getByRole("button", { name: "Keep saved" }));
  expect(screen.getByText("Keep saved credential")).toBeInTheDocument();
  fireEvent.click(screen.getByRole("button", { name: "Clear" }));
  expect(screen.getByText("Clear on save")).toBeInTheDocument();
});

it("discards a reveal completed after hide and shows a fixed error message", async () => {
  let finish!: (value: string) => void;
  const reveal = vi.fn(() => new Promise<string>((resolve) => { finish = resolve; }));
  const props = { saved: true, cleared: false, value: "", onChange: vi.fn(), onClear: vi.fn(), onKeep: vi.fn() };
  const { rerender } = render(<SavedSecretInput {...props} onReveal={reveal} />);
  fireEvent.click(screen.getByRole("button", { name: "Show secret value" }));
  fireEvent.click(screen.getByRole("button", { name: "Hide secret value" }));
  finish("late-secret");
  await waitFor(() => expect(screen.queryByDisplayValue("late-secret")).toBeNull());
  rerender(<SavedSecretInput {...props} onReveal={async () => { throw new Error("error-containing-secret"); }} />);
  fireEvent.click(screen.getByRole("button", { name: "Show secret value" }));
  expect(await screen.findByRole("alert")).toHaveTextContent("Unable to show the saved credential.");
  expect(screen.queryByText("error-containing-secret")).toBeNull();
});
