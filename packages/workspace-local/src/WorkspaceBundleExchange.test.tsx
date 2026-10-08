// @vitest-environment jsdom
import { QueryClient, QueryClientProvider } from "@tanstack/react-query";
import { cleanup, fireEvent, render, screen, waitFor } from "@testing-library/react";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import { exportWorkspaceBundle, importWorkspaceBundle, pickWorkspaceBundle, type Workspace } from "@unfour/command-client";
import { useWorkspaceBundleExchange } from "./WorkspaceBundleExchange";

vi.mock("@unfour/command-client", () => ({
  exportWorkspaceBundle: vi.fn(), importWorkspaceBundle: vi.fn(), pickWorkspaceBundle: vi.fn(),
}));
const file = {
  content: '{"format":"unfour-workspace"}',
  preview: { name: "Example (Copy 1)", counts: { requests: 3, flows: 1 }, reconfigure: [{ entityId: "ssh-old", name: "Deploy host", code: "connection" }] },
};
function Harness({ imported, target = { id: "current", name: "Current project", environmentType: "test" } }: {
  imported: (id: string) => void;
  target?: Pick<Workspace, "id" | "name" | "environmentType">;
}) {
  const exchange = useWorkspaceBundleExchange(imported);
  return <>
    <button onClick={() => void exchange.pick()} disabled={exchange.busy}>Pick file</button>
    <button onClick={() => exchange.exportWorkspace(target)} disabled={exchange.busy}>Export current</button>
    {exchange.dialog}
  </>;
}
function mount() {
  const imported = vi.fn();
  const client = new QueryClient({ defaultOptions: { queries: { retry: false }, mutations: { retry: false } } });
  const invalidate = vi.spyOn(client, "invalidateQueries");
  render(<QueryClientProvider client={client}><Harness imported={imported} /></QueryClientProvider>);
  return { imported, invalidate };
}
afterEach(cleanup);
beforeEach(() => {
  vi.clearAllMocks();
  vi.mocked(pickWorkspaceBundle).mockResolvedValue(file);
  vi.mocked(exportWorkspaceBundle).mockResolvedValue({ saved: true });
});
describe("Workspace bundle exchange", () => {
  it("previews counts and reconfiguration, then imports the reviewed content with an edited name", async () => {
    const { imported, invalidate } = mount();
    fireEvent.click(screen.getByText("Pick file"));
    expect(await screen.findByRole("dialog")).toBeTruthy();
    expect(screen.getByText("API requests")).toBeTruthy();
    expect(screen.getByText("Deploy host")).toBeTruthy();
    expect(importWorkspaceBundle).not.toHaveBeenCalled();
    const name = screen.getByLabelText("New Workspace name");
    expect(name).toHaveValue("Example (Copy 1)");
    fireEvent.change(name, { target: { value: "  New copy  " } });
    let finish: (workspace: Workspace) => void = () => {};
    vi.mocked(importWorkspaceBundle).mockImplementation(() => new Promise((resolve) => { finish = resolve; }));
    fireEvent.click(screen.getByRole("button", { name: "Create and Switch" }));
    expect(await screen.findByRole("button", { name: "Working…" })).toBeDisabled();
    expect(screen.getByRole("button", { name: "Cancel" })).toBeDisabled();
    expect(importWorkspaceBundle).toHaveBeenCalledExactlyOnceWith(file.content, "New copy");
    finish({ id: "new-workspace", name: "New copy", environmentType: "dev", mcpPolicy: "disabled", isDefault: false, lastOpenedAt: null, createdAt: "2026-10-01T00:00:00Z", updatedAt: "2026-10-01T00:00:00Z", deletedAt: null, revision: 1 });
    await waitFor(() => expect(imported).toHaveBeenCalledWith("new-workspace"));
    expect(invalidate).toHaveBeenCalledWith({ queryKey: ["workspaces"] });
    expect(screen.queryByRole("dialog")).toBeNull();
  });
  it("cancels without creating or changing a workspace", async () => {
    const { imported } = mount();
    fireEvent.click(screen.getByText("Pick file"));
    fireEvent.click(await screen.findByRole("button", { name: "Cancel" }));
    expect(importWorkspaceBundle).not.toHaveBeenCalled();
    expect(imported).not.toHaveBeenCalled();
    vi.mocked(pickWorkspaceBundle).mockResolvedValueOnce(null);
    fireEvent.click(screen.getByText("Pick file"));
    await waitFor(() => expect(screen.getByText("Pick file")).not.toBeDisabled());
    expect(screen.queryByRole("dialog")).toBeNull();
  });
  it("keeps preview open and allows retry after an import failure", async () => {
    const { imported } = mount();
    vi.mocked(importWorkspaceBundle).mockRejectedValueOnce(new Error("WORKSPACE_BUNDLE_MISSING_REFERENCE"));
    const log = vi.spyOn(console, "error").mockImplementation(() => {});
    fireEvent.click(screen.getByText("Pick file"));
    fireEvent.click(await screen.findByRole("button", { name: "Create and Switch" }));
    await waitFor(() => expect(screen.getByRole("button", { name: "Create and Switch" })).not.toBeDisabled());
    expect(screen.getByRole("dialog")).toBeTruthy();
    expect(imported).not.toHaveBeenCalled();
    log.mockRestore();
  });
  it("only exports the selected workspace after reviewing the exclusions", async () => {
    mount();
    fireEvent.click(screen.getByText("Export current"));
    expect(exportWorkspaceBundle).not.toHaveBeenCalled();
    expect(screen.getByRole("dialog", { name: "Export Workspace" })).toHaveTextContent("Current project");
    expect(screen.getByText("TEST")).toBeTruthy();
    expect(screen.getByText(/History, UI state, Cloud Sync state/)).toBeTruthy();
    fireEvent.click(screen.getByRole("button", { name: "Choose Save Location…" }));
    await waitFor(() => expect(exportWorkspaceBundle).toHaveBeenCalledExactlyOnceWith("current"));
    await waitFor(() => expect(screen.queryByRole("dialog")).toBeNull());
  });
  it("keeps the reviewed export target when the active workspace changes", async () => {
    const client = new QueryClient();
    const imported = vi.fn();
    const { rerender } = render(<QueryClientProvider client={client}><Harness imported={imported} /></QueryClientProvider>);
    fireEvent.click(screen.getByText("Export current"));
    rerender(<QueryClientProvider client={client}><Harness imported={imported} target={{ id: "other", name: "Other project", environmentType: "prod" }} /></QueryClientProvider>);
    expect(screen.getByRole("dialog")).toHaveTextContent("Current project");
    expect(screen.getByRole("dialog")).not.toHaveTextContent("Other project");
    fireEvent.click(screen.getByRole("button", { name: "Choose Save Location…" }));
    await waitFor(() => expect(exportWorkspaceBundle).toHaveBeenCalledExactlyOnceWith("current"));
  });
});
