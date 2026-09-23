// @vitest-environment jsdom
import "@testing-library/jest-dom/vitest";
import { cleanup, fireEvent, render, screen, waitFor } from "@testing-library/react";
import { afterEach, expect, it, vi } from "vitest";
import type { DatabaseConnection, DatabaseTable } from "@unfour/command-client";
import { TableExportDialog } from "./TableExportDialog";

const { save, exportDatabaseTable } = vi.hoisted(() => ({
  save: vi.fn().mockResolvedValue("C:\\exports\\users.csv"),
  exportDatabaseTable: vi.fn().mockResolvedValue({ path: "C:\\exports\\users.csv", rowCount: 1500, bytesWritten: 8000, format: "csv" }),
}));
vi.mock("@tauri-apps/plugin-dialog", () => ({ save }));
vi.mock("@unfour/command-client", () => ({ exportDatabaseTable }));
afterEach(() => { cleanup(); vi.clearAllMocks(); });

it("exports the entire selected table without pagination parameters", async () => {
  const connection = { id: "conn-1", workspaceId: "ws-1", driver: "sqlite" } as DatabaseConnection;
  const table = { name: "users", catalog: null, schema: null, kind: "table", columns: [] } as DatabaseTable;
  render(<TableExportDialog connection={connection} onOpenChange={vi.fn()} table={table} />);
  fireEvent.change(screen.getByLabelText("Content"), { target: { value: "data" } });
  fireEvent.change(screen.getByLabelText("Format"), { target: { value: "csv" } });
  fireEvent.click(screen.getByRole("button", { name: "Choose…" }));
  await waitFor(() => expect(save).toHaveBeenCalled());
  fireEvent.click(screen.getByRole("button", { name: "Export" }));
  await waitFor(() => expect(exportDatabaseTable).toHaveBeenCalledWith({
    workspaceId: "ws-1", connectionId: "conn-1", catalog: null, schema: null,
    tableName: "users", content: "data", format: "csv", destinationPath: "C:\\exports\\users.csv",
  }));
  expect(screen.getByRole("status")).toHaveTextContent("1500");
});
