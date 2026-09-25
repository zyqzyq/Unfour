// @vitest-environment jsdom
import "@testing-library/jest-dom/vitest";
import { useState } from "react";
import { cleanup, fireEvent, render, screen } from "@testing-library/react";
import { afterEach, expect, it, vi } from "vitest";
import type { DatabaseConnectionInput } from "@unfour/command-client";
import { DatabaseConnectionDialog } from "./DatabaseConnectionDialog";
import { DatabaseTestResultDialog } from "./DatabaseTestResultDialog";

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
  expect(screen.getAllByRole("option").map((option) => option.textContent)).toEqual(["PostgreSQL", "openGauss", "MySQL", "SQLite"]);
  fireEvent.change(screen.getByRole("combobox"), { target: { value: "opengauss" } });
  expect(screen.getByRole("combobox")).toHaveValue("opengauss");
  fireEvent.click(screen.getByRole("button", { name: "Save" }));
  expect(save).toHaveBeenCalledWith({ workspaceId: "ws", name: "Gauss", driver: "postgres", sqlitePath: null, sslMode: undefined, credentialRef: undefined });
  expect(JSON.stringify(save.mock.calls[0][0])).not.toContain("opengauss");
  fireEvent.click(screen.getByRole("button", { name: "Reopen" }));
  expect(screen.getByRole("combobox")).toHaveValue("postgres");
});

it("shows runtime detection separately from the protocol and raw banner", () => {
  render(<DatabaseTestResultDialog onOpenChange={() => {}} result={{ ok: true, message: "PostgreSQL connection OK", protocol: "postgres", detectedServer: "openGauss", serverVersion: "PostgreSQL 9.2.4 (openGauss 5.0.0)" }} />);
  expect(screen.getByText("Protocol: PostgreSQL")).toBeInTheDocument();
  expect(screen.getByText("Detected server: openGauss")).toBeInTheDocument();
  expect(screen.getByText("PostgreSQL 9.2.4 (openGauss 5.0.0)")).toBeInTheDocument();
});
