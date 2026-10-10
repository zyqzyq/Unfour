// @vitest-environment jsdom
import { QueryClient, QueryClientProvider } from "@tanstack/react-query";
import { cleanup, fireEvent, render, screen, waitFor } from "@testing-library/react";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import { updateWorkspaceEnvironment, updateWorkspaceMcpPolicy, type Workspace, type WorkspaceState } from "@unfour/command-client";
import { WorkspaceSecurityDialog } from "./WorkspaceSecurityDialog";

vi.mock("@unfour/command-client", () => ({ updateWorkspaceEnvironment: vi.fn(), updateWorkspaceMcpPolicy: vi.fn() }));
const imported: Workspace = {
  id: "imported", name: "Imported", environmentType: "dev", mcpPolicy: "disabled",
  isDefault: false, lastOpenedAt: null, deletedAt: null, revision: 1,
  createdAt: "2026-10-10T00:00:00Z", updatedAt: "2026-10-10T00:00:00Z",
};
function mount(workspace = imported) {
  const client = new QueryClient({ defaultOptions: { mutations: { retry: false } } });
  client.setQueryData<WorkspaceState>(["workspaces"], { activeWorkspaceId: workspace.id, workspaces: [workspace] });
  const close = vi.fn();
  render(<QueryClientProvider client={client}><WorkspaceSecurityDialog workspace={workspace} onClose={close} /></QueryClientProvider>);
  return { client, close };
}
beforeEach(() => vi.resetAllMocks());
afterEach(cleanup);

describe("Workspace security settings", () => {
  it("lets an imported disabled workspace save permissions without saving an environment draft", async () => {
    const { client, close } = mount();
    expect(screen.getByLabelText("MCP permissions")).toHaveValue("disabled");
    expect(screen.getAllByRole("option").map((item) => item.getAttribute("value"))).toEqual(["dev", "test", "prod", "auto", "disabled", "read_only", "guarded", "full_access"]);
    fireEvent.change(screen.getByLabelText("Workspace environment type"), { target: { value: "prod" } });
    fireEvent.change(screen.getByLabelText("MCP permissions"), { target: { value: "full_access" } });
    vi.mocked(updateWorkspaceMcpPolicy).mockResolvedValue({ ...imported, mcpPolicy: "full_access", revision: 2 });
    fireEvent.click(screen.getByRole("button", { name: "Save MCP permissions" }));
    await waitFor(() => expect(screen.getByText("Saved permissions: Full access (full_access)")).toBeTruthy());
    expect(updateWorkspaceMcpPolicy).toHaveBeenCalledExactlyOnceWith("imported", "full_access");
    expect(updateWorkspaceEnvironment).not.toHaveBeenCalled();
    expect(screen.getByLabelText("Workspace environment type")).toHaveValue("prod");
    expect(client.getQueryData<WorkspaceState>(["workspaces"])?.workspaces[0]).toMatchObject({ environmentType: "dev", mcpPolicy: "full_access" });
    expect(close).not.toHaveBeenCalled();
  });

  it("saves the environment independently and keeps an unsaved MCP selection", async () => {
    const { client } = mount();
    vi.mocked(updateWorkspaceEnvironment).mockResolvedValue({ ...imported, environmentType: "test", revision: 2 });
    fireEvent.change(screen.getByLabelText("MCP permissions"), { target: { value: "auto" } });
    fireEvent.change(screen.getByLabelText("Workspace environment type"), { target: { value: "test" } });
    expect(screen.getByText("Auto currently resolves to Full access (full_access) using the saved environment type.")).toBeTruthy();
    fireEvent.click(screen.getByRole("button", { name: "Save environment" }));
    await waitFor(() => expect(screen.getByText("Auto currently resolves to Guarded (guarded) using the saved environment type.")).toBeTruthy());
    expect(updateWorkspaceEnvironment).toHaveBeenCalledExactlyOnceWith("imported", "test");
    expect(updateWorkspaceMcpPolicy).not.toHaveBeenCalled();
    expect(screen.getByLabelText("MCP permissions")).toHaveValue("auto");
    expect(client.getQueryData<WorkspaceState>(["workspaces"])?.workspaces[0]).toMatchObject({ environmentType: "test", mcpPolicy: "disabled" });
  });

  it.each([["dev", "Full access (full_access)"], ["test", "Guarded (guarded)"], ["prod", "Read-only (read_only)"]] as const)(
    "shows saved Auto permissions for %s", (environmentType, effective) => {
      mount({ ...imported, environmentType, mcpPolicy: "auto" });
      expect(screen.getByLabelText("MCP permissions")).toHaveValue("auto");
      expect(screen.getByText(`Auto currently resolves to ${effective} using the saved environment type.`)).toBeTruthy();
      fireEvent.change(screen.getByLabelText("MCP permissions"), { target: { value: "disabled" } });
      expect(screen.getByText(`Saved permissions: Auto (auto) → ${effective}`)).toBeTruthy();
    },
  );

  it("retains permissions after a failed save and retries without changing persisted state prematurely", async () => {
    const { client, close } = mount();
    vi.mocked(updateWorkspaceMcpPolicy).mockRejectedValueOnce(new Error("disk write failed"));
    fireEvent.change(screen.getByLabelText("MCP permissions"), { target: { value: "read_only" } });
    fireEvent.click(screen.getByRole("button", { name: "Save MCP permissions" }));
    expect(await screen.findByRole("alert")).toHaveTextContent("Failed to save MCP permissions");
    expect(screen.getByLabelText("MCP permissions")).toHaveValue("read_only");
    expect(client.getQueryData<WorkspaceState>(["workspaces"])?.workspaces[0].mcpPolicy).toBe("disabled");
    expect(close).not.toHaveBeenCalled();
    vi.mocked(updateWorkspaceMcpPolicy).mockResolvedValue({ ...imported, mcpPolicy: "read_only", revision: 2 });
    fireEvent.click(screen.getByRole("button", { name: "Save MCP permissions" }));
    await waitFor(() => expect(screen.getByText("Saved permissions: Read-only (read_only)")).toBeTruthy());
    expect(screen.queryByRole("alert")).toBeNull();
    expect(updateWorkspaceMcpPolicy).toHaveBeenCalledTimes(2);
  });

  it("keeps the environment draft after a failed save", async () => {
    const { client } = mount();
    vi.mocked(updateWorkspaceEnvironment).mockRejectedValueOnce(new Error("disk write failed"));
    fireEvent.change(screen.getByLabelText("Workspace environment type"), { target: { value: "prod" } });
    fireEvent.click(screen.getByRole("button", { name: "Save environment" }));
    expect(await screen.findByRole("alert")).toHaveTextContent("Failed to save the workspace environment type");
    expect(screen.getByLabelText("Workspace environment type")).toHaveValue("prod");
    expect(client.getQueryData<WorkspaceState>(["workspaces"])?.workspaces[0]).toEqual(imported);
    expect(screen.getByRole("button", { name: "Save environment" })).not.toBeDisabled();
  });

  it("prevents duplicate saves and closing while permissions are pending", async () => {
    const { close } = mount();
    let finish!: (workspace: Workspace) => void;
    vi.mocked(updateWorkspaceMcpPolicy).mockImplementation(() => new Promise((resolve) => { finish = resolve; }));
    fireEvent.change(screen.getByLabelText("MCP permissions"), { target: { value: "guarded" } });
    const save = screen.getByRole("button", { name: "Save MCP permissions" });
    fireEvent.click(save);
    await waitFor(() => expect(save).toBeDisabled());
    expect(screen.getByRole("button", { name: "Close" })).toBeDisabled();
    expect(screen.getByLabelText("MCP permissions")).toBeDisabled();
    fireEvent.click(save);
    expect(updateWorkspaceMcpPolicy).toHaveBeenCalledTimes(1);
    expect(close).not.toHaveBeenCalled();
    finish({ ...imported, mcpPolicy: "guarded" });
    await waitFor(() => expect(screen.getByRole("button", { name: "Close" })).not.toBeDisabled());
  });
});
