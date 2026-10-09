// @vitest-environment jsdom
import "@testing-library/jest-dom/vitest";
import { useState } from "react";
import { cleanup, fireEvent, render, screen } from "@testing-library/react";
import { afterEach, expect, it, vi } from "vitest";
import { revealConnectionSecret, type DatabaseConnectionInput } from "@unfour/command-client";
import { DatabaseConnectionDialog } from "./DatabaseConnectionDialog";
import { DatabaseTestResultDialog } from "./DatabaseTestResultDialog";

vi.mock("@unfour/command-client", async (importOriginal) => ({
  ...await importOriginal<typeof import("@unfour/command-client")>(),
  revealConnectionSecret: vi.fn().mockResolvedValue("saved-db-secret"),
}));

afterEach(cleanup);

it("saves the openGauss preset as postgres and reopens from the saved driver", () => {
  const save = vi.fn();
  function Editor() {
    const [open, setOpen] = useState(true);
    const [form, setForm] = useState<DatabaseConnectionInput>({ workspaceId: "ws", name: "Gauss", driver: "postgres" });
    return <>
      <button onClick={() => setOpen(true)}>Reopen</button>
      <DatabaseConnectionDialog canTest error={null} form={form} open={open}
        onOpenChange={setOpen} onPasswordChange={() => {}} password=""
        onSubmit={(event) => { event.preventDefault(); save(form); setOpen(false); }}
        onTest={() => {}} onUpdate={(patch) => setForm((current) => ({ ...current, ...patch }))}
        savePending={false} testPending={false} />
    </>;
  }
  render(<Editor />);
  expect(screen.getAllByRole("option").map((option) => option.textContent)).toEqual(["PostgreSQL", "openGauss", "MySQL / MariaDB", "SQLite"]);
  fireEvent.change(screen.getByRole("combobox"), { target: { value: "opengauss" } });
  expect(screen.getByRole("combobox")).toHaveValue("opengauss");
  fireEvent.click(screen.getByRole("button", { name: "Save" }));
  expect(save).toHaveBeenCalledWith({ workspaceId: "ws", name: "Gauss", driver: "postgres", sqlitePath: null, sslMode: undefined, credentialRef: undefined });
  expect(JSON.stringify(save.mock.calls[0][0])).not.toContain("opengauss");
  fireEvent.click(screen.getByRole("button", { name: "Reopen" }));
  expect(screen.getByRole("combobox")).toHaveValue("postgres");
});

it("shows runtime detection separately from the protocol and raw banner", () => {
  render(<DatabaseTestResultDialog onOpenChange={() => {}} result={{ ok: true, message: "openGauss connection OK", protocol: "postgres", detectedServer: "openGauss", serverVersion: "PostgreSQL 9.2.4 (openGauss 6.0.3)" }} />);
  expect(screen.getByText("openGauss connection OK")).toBeInTheDocument();
  expect(screen.getByText("Protocol: PostgreSQL")).toBeInTheDocument();
  expect(screen.getByText("Detected server: openGauss")).toBeInTheDocument();
  expect(screen.getByText("PostgreSQL 9.2.4 (openGauss 6.0.3)")).toBeInTheDocument();
});

it("views a saved database password without saving plaintext and keeps clear reversible", async () => {
  const save = vi.fn();
  function Editor() {
    const [form, setForm] = useState<DatabaseConnectionInput>({ workspaceId: "ws", id: "db", name: "Database", driver: "postgres", credentialRef: "saved-ref" });
    const [password, setPassword] = useState("");
    return <DatabaseConnectionDialog canTest error={null} form={form} open onOpenChange={() => {}}
      onPasswordChange={setPassword} password={password} onSubmit={(event) => { event.preventDefault(); save(form, password); }}
      onTest={() => {}} onUpdate={(patch) => setForm((current) => ({ ...current, ...patch }))} savePending={false} testPending={false} />;
  }
  render(<Editor />);
  fireEvent.click(screen.getByRole("button", { name: "Show secret value" }));
  await screen.findByDisplayValue("saved-db-secret");
  expect(revealConnectionSecret).toHaveBeenCalledWith({ workspaceId: "ws", connectionId: "db", connectionType: "database", credentialRef: "saved-ref" });
  fireEvent.click(screen.getByRole("button", { name: "Save" }));
  expect(save.mock.lastCall).toEqual([expect.objectContaining({ credentialRef: "saved-ref" }), ""]);
  fireEvent.click(screen.getByRole("button", { name: "Clear" }));
  fireEvent.click(screen.getByRole("button", { name: "Save" }));
  expect(save.mock.lastCall).toEqual([expect.objectContaining({ credentialRef: null }), ""]);
  fireEvent.click(screen.getByRole("button", { name: "Keep saved" }));
  fireEvent.change(screen.getByPlaceholderText("Saved credential; type to replace"), { target: { value: "replacement" } });
  fireEvent.click(screen.getByRole("button", { name: "Save" }));
  expect(save.mock.lastCall).toEqual([expect.objectContaining({ credentialRef: "saved-ref" }), "replacement"]);
});
