// @vitest-environment jsdom
import { useState } from "react";
import { cleanup, fireEvent, render, screen } from "@testing-library/react";
import { afterEach, expect, it, vi } from "vitest";
import { listTemporaryRouteHandles, revealConnectionSecret, type SshConnectionInput } from "@unfour/command-client";
import { SshConnectionDialog } from "./SshConnectionDialog";

vi.mock("@unfour/command-client", async (importOriginal) => ({
  ...await importOriginal<typeof import("@unfour/command-client")>(),
  listTemporaryRouteHandles: vi.fn().mockResolvedValue([]),
  revealConnectionSecret: vi.fn().mockResolvedValue("saved-ssh-secret"),
}));
afterEach(cleanup);

it("selects an active bastion session for the target SSH connection", async () => {
  vi.mocked(listTemporaryRouteHandles).mockResolvedValueOnce(["session-12345678"]);
  const update = vi.fn();
  render(<SshConnectionDialog canTest open onOpenChange={vi.fn()}
    form={{ workspaceId: "ws", name: "Host", host: "target", username: "dev", authKind: "none" }}
    onSubmit={vi.fn()} onTest={vi.fn()} onUpdate={update} />);
  await screen.findByRole("option", { name: "Bastion session · 12345678" });
  fireEvent.change(screen.getByLabelText("Network route"), { target: { value: "session-12345678" } });
  expect(update).toHaveBeenCalledWith({ temporaryHandle: "session-12345678" });
});

it("prevents editing and closing while a connection save is pending", () => {
  const close = vi.fn();
  render(<SshConnectionDialog canTest pending open onOpenChange={close}
    form={{ workspaceId: "ws", name: "Host", host: "fixture", username: "dev", authKind: "none" }}
    onSubmit={vi.fn()} onTest={vi.fn()} onUpdate={vi.fn()} />);
  expect(screen.getByRole("textbox", { name: "Name" })).toBeDisabled();
  expect(screen.getByRole("button", { name: "Cancel" })).toBeDisabled();
  fireEvent.keyDown(screen.getByRole("dialog"), { key: "Escape" });
  expect(close).not.toHaveBeenCalled();
});

it.each(["password", "private-key"] as const)("keeps %s viewing separate from preserve, replace and clear save intents", async (authKind) => {
  const save = vi.fn();
  function Editor() {
    const [form, setForm] = useState<SshConnectionInput>({ workspaceId: "ws", id: "ssh", name: "Host", host: "localhost", username: "user", authKind, keyPath: "C:/fixture/key", credentialRef: "saved-ref", secret: null });
    return <SshConnectionDialog canTest form={form} open onOpenChange={() => {}}
      onSubmit={(event) => { event.preventDefault(); save(form); }} onTest={() => {}}
      onUpdate={(patch) => setForm((current) => ({ ...current, ...patch }))} />;
  }
  render(<Editor />);
  fireEvent.click(screen.getByRole("button", { name: "Show secret value" }));
  await screen.findByDisplayValue("saved-ssh-secret");
  expect(revealConnectionSecret).toHaveBeenCalledWith({ workspaceId: "ws", connectionId: "ssh", connectionType: "ssh", credentialRef: "saved-ref" });
  fireEvent.click(screen.getByRole("button", { name: "Save" }));
  expect(save.mock.lastCall?.[0]).toMatchObject({ credentialRef: "saved-ref", secret: null });
  fireEvent.click(screen.getByRole("button", { name: "Clear" }));
  fireEvent.click(screen.getByRole("button", { name: "Save" }));
  expect(save.mock.lastCall?.[0]).toMatchObject({ credentialRef: null, secret: null });
  fireEvent.click(screen.getByRole("button", { name: "Keep saved" }));
  fireEvent.change(screen.getByPlaceholderText("Saved credential; type to replace"), { target: { value: "replacement" } });
  fireEvent.click(screen.getByRole("button", { name: "Save" }));
  expect(save.mock.lastCall?.[0]).toMatchObject({ credentialRef: "saved-ref", secret: "replacement" });
});
