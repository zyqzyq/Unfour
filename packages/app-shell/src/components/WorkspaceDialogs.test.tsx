// @vitest-environment jsdom
import { QueryClient, QueryClientProvider } from "@tanstack/react-query";
import { cleanup, fireEvent, render, screen, waitFor } from "@testing-library/react";
import { afterEach, expect, it, vi } from "vitest";
import { I18nProvider } from "@unfour/ui";
import type { Workspace } from "@unfour/command-client";
import { renameWorkspace } from "@unfour/command-client";
import { WorkspaceDialogs } from "./WorkspaceDialogs";
vi.mock("@unfour/workspace-local", () => ({ WorkspaceSecurityDialog: () => null }));
vi.mock("@unfour/command-client", () => ({ createWorkspace: vi.fn(), deleteWorkspace: vi.fn(), renameWorkspace: vi.fn() }));
afterEach(cleanup);
const workspace: Workspace = { id: "w", name: "Old", environmentType: "dev", mcpPolicy: "auto", isDefault: true, lastOpenedAt: null, createdAt: "t", updatedAt: "t", deletedAt: null, revision: 1 };
function mount() {
  const client = new QueryClient();
  const view = (activeWorkspace: Workspace) => <I18nProvider initialLocale="en"><QueryClientProvider client={client}><WorkspaceDialogs activeWorkspace={activeWorkspace} workspaces={[activeWorkspace]} createOpen={false} deleteOpen={false} environmentOpen={false} renameOpen onCreateClose={vi.fn()} onDeleteClose={vi.fn()} onEnvironmentClose={vi.fn()} onRenameClose={vi.fn()} /></QueryClientProvider></I18nProvider>;
  const rendered = render(view(workspace));
  return () => rendered.rerender(view({ ...workspace, name: "External", revision: 2 }));
}
it("refreshes an untouched workspace rename draft", () => {
  const refresh = mount();
  refresh();
  expect(screen.getByDisplayValue("External")).toBeTruthy();
});
it("retains a conflicting workspace rename draft until reload", () => {
  const refresh = mount();
  fireEvent.change(screen.getByDisplayValue("Old"), { target: { value: "Local" } });
  refresh();
  expect(screen.getByDisplayValue("Local")).toBeTruthy();
  expect(screen.getByRole("button", {name: "Rename"})).toBeDisabled();
  fireEvent.click(screen.getByRole("button", {name: "Load latest version"}));
  expect(screen.getByDisplayValue("External")).toBeTruthy();
});
it("prevents typing and dismissal while a rename is pending", async () => {
  vi.mocked(renameWorkspace).mockImplementationOnce(() => new Promise(() => {}));
  mount();
  fireEvent.change(screen.getByDisplayValue("Old"), { target: { value: "New" } });
  fireEvent.click(screen.getByRole("button", { name: "Rename" }));
  await waitFor(() => expect(screen.getByRole("button", { name: "Rename" })).toBeDisabled());
  expect(screen.getByDisplayValue("New")).toBeDisabled();
  expect(screen.getByRole("button", { name: "Cancel" })).toBeDisabled();
});
