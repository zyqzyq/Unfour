// @vitest-environment jsdom
import { QueryClient, QueryClientProvider } from "@tanstack/react-query";
import { cleanup, fireEvent, render, screen, waitFor, within } from "@testing-library/react";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import { updateWorkspaceEnvironment, updateWorkspaceMcpPolicy, type Workspace, type WorkspaceState } from "@unfour/command-client";
import { I18nProvider, type Locale } from "@unfour/ui";
import { WorkspaceSecurityDialog } from "./WorkspaceSecurityDialog";

vi.mock("@unfour/command-client", () => ({ updateWorkspaceEnvironment: vi.fn(), updateWorkspaceMcpPolicy: vi.fn() }));
const imported: Workspace = {
  id: "imported", name: "Imported", environmentType: "dev", mcpPolicy: "disabled",
  isDefault: false, lastOpenedAt: null, deletedAt: null, revision: 1,
  createdAt: "2026-10-10T00:00:00Z", updatedAt: "2026-10-10T00:00:00Z",
};
function mount(workspace = imported, locale: Locale = "en") {
  const client = new QueryClient({ defaultOptions: { mutations: { retry: false } } });
  client.setQueryData<WorkspaceState>(["workspaces"], { activeWorkspaceId: workspace.id, workspaces: [workspace] });
  const close = vi.fn();
  render(<I18nProvider initialLocale={locale}><QueryClientProvider client={client}><WorkspaceSecurityDialog workspace={workspace} onClose={close} /></QueryClientProvider></I18nProvider>);
  return { client, close };
}
beforeEach(() => vi.resetAllMocks());
afterEach(cleanup);

describe("Workspace security settings", () => {
  it.each(["en", "zh-CN"] as const)("previews environment saves using the saved policy rather than its draft in %s", (locale) => {
    mount({ ...imported, mcpPolicy: "auto" }, locale);
    const environment = screen.getByLabelText(locale === "en" ? "Workspace environment type" : "工作区环境类型");
    const policy = screen.getByLabelText(locale === "en" ? "MCP permissions" : "MCP 权限");
    const environmentForm = within(environment.closest("form")!);
    const policyForm = within(policy.closest("form")!);
    const autoWarning = locale === "en"
      ? "PROD with Auto uses read-only MCP defaults. Write and execution actions are blocked."
      : "PROD + Auto 的 MCP 默认只读，写入和执行类操作会被拦截。";
    expect(screen.queryByRole("status")).toBeNull();
    fireEvent.change(environment, { target: { value: "prod" } });
    expect(environmentForm.getByRole("status")).toHaveTextContent(autoWarning);
    expect(environmentForm.getByRole("status")).toHaveTextContent(locale === "en"
      ? "Saving the environment uses the saved MCP permissions."
      : "保存环境类型时沿用已保存的 MCP 权限。");
    for (const value of ["guarded", "full_access", "auto", "disabled", "read_only"]) {
      fireEvent.change(policy, { target: { value } });
      expect(environmentForm.getByRole("status")).toHaveTextContent(autoWarning);
      expect(policyForm.queryByRole("status")).toBeNull();
    }
    fireEvent.change(policy, { target: { value: "guarded" } });
    for (const value of ["test", "dev"]) {
      fireEvent.change(environment, { target: { value } });
      expect(screen.queryByRole("status")).toBeNull();
    }
    expect(screen.getAllByRole("dialog")).toHaveLength(1);
    expect(updateWorkspaceEnvironment).not.toHaveBeenCalled();
    expect(updateWorkspaceMcpPolicy).not.toHaveBeenCalled();
  });

  it.each(["guarded", "full_access"] as const)("warns immediately for saved PROD with %s", (mcpPolicy) => {
    mount({ ...imported, environmentType: "prod", mcpPolicy });
    for (const notice of screen.getAllByRole("status")) {
      expect(notice).toHaveTextContent("overrides the production environment's default protection");
      expect(notice).toHaveTextContent("may allow an Agent to modify real resources");
    }
    expect(screen.getByRole("button", { name: "Save MCP permissions" })).toBeDisabled();
  });

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

describe.each(["en", "zh-CN"] as const)("Independent security save risk previews in %s", (locale) => {
  const labels = locale === "en" ? {
    environment: "Workspace environment type", policy: "MCP permissions",
    saveEnvironment: "Save environment", savePolicy: "Save MCP permissions",
    savedProd: "Saved environment: PROD", savedDev: "Saved environment: DEV",
    savedDisabled: "Saved permissions: Disabled (disabled)", savedFull: "Saved permissions: Full access (full_access)",
    policyContext: "Saving MCP permissions uses the saved environment type.",
    auto: "PROD with Auto uses read-only MCP defaults.",
    guarded: "PROD with Guarded (guarded) overrides the production environment's default protection",
    full: "PROD with Full access (full_access) overrides the production environment's default protection",
  } : {
    environment: "工作区环境类型", policy: "MCP 权限",
    saveEnvironment: "保存环境类型", savePolicy: "保存 MCP 权限",
    savedProd: "已保存环境：PROD", savedDev: "已保存环境：DEV",
    savedDisabled: "已保存权限：禁用 (disabled)", savedFull: "已保存权限：完全访问 (full_access)",
    policyContext: "保存 MCP 权限时沿用已保存的环境类型。",
    auto: "PROD + Auto 的 MCP 默认只读",
    guarded: "PROD + 受保护 (guarded) 会覆盖生产环境的默认保护",
    full: "PROD + 完全访问 (full_access) 会覆盖生产环境的默认保护",
  };

  it("warns for a permission-only save on saved PROD even with an unsaved DEV or TEST environment", async () => {
    const workspace = { ...imported, environmentType: "prod" as const };
    const { client, close } = mount(workspace, locale);
    const environment = screen.getByLabelText(labels.environment);
    const policy = screen.getByLabelText(labels.policy);
    const environmentForm = within(environment.closest("form")!);
    const policyForm = within(policy.closest("form")!);
    for (const draft of ["test", "dev"]) {
      fireEvent.change(environment, { target: { value: draft } });
      for (const [value, warning] of [["guarded", labels.guarded], ["full_access", labels.full], ["auto", labels.auto]]) {
        fireEvent.change(policy, { target: { value } });
        expect(policyForm.getByRole("status")).toHaveTextContent(warning);
        expect(policyForm.getByRole("status")).toHaveTextContent(labels.policyContext);
        expect(environmentForm.queryByRole("status")).toBeNull();
      }
      for (const value of ["disabled", "read_only"]) {
        fireEvent.change(policy, { target: { value } });
        expect(screen.queryByRole("status")).toBeNull();
      }
    }
    expect(screen.getByText(labels.savedProd)).toBeTruthy();
    expect(screen.getByText(labels.savedDisabled)).toBeTruthy();
    fireEvent.change(policy, { target: { value: "full_access" } });
    vi.mocked(updateWorkspaceMcpPolicy).mockResolvedValue({ ...workspace, mcpPolicy: "full_access", revision: 2 });
    fireEvent.click(screen.getByRole("button", { name: labels.savePolicy }));
    await waitFor(() => expect(screen.getByText(labels.savedFull)).toBeTruthy());
    expect(policyForm.getByRole("status")).toHaveTextContent(labels.full);
    expect(environment).toHaveValue("dev");
    expect(updateWorkspaceMcpPolicy).toHaveBeenCalledExactlyOnceWith(workspace.id, "full_access");
    expect(updateWorkspaceEnvironment).not.toHaveBeenCalled();
    expect(client.getQueryData<WorkspaceState>(["workspaces"])?.workspaces[0]).toMatchObject({ environmentType: "prod", mcpPolicy: "full_access" });
    expect(close).not.toHaveBeenCalled();

    vi.mocked(updateWorkspaceEnvironment).mockResolvedValue({ ...workspace, environmentType: "dev", mcpPolicy: "full_access", revision: 3 });
    fireEvent.click(screen.getByRole("button", { name: labels.saveEnvironment }));
    await waitFor(() => expect(screen.getByText(labels.savedDev)).toBeTruthy());
    expect(screen.queryByRole("status")).toBeNull();
    expect(policy).toHaveValue("full_access");
  });

  it("warns for an environment-only save using saved full access even with a safer permission draft, including retry", async () => {
    const workspace = { ...imported, mcpPolicy: "full_access" as const };
    const { client } = mount(workspace, locale);
    const environment = screen.getByLabelText(labels.environment);
    const policy = screen.getByLabelText(labels.policy);
    const environmentForm = within(environment.closest("form")!);
    const policyForm = within(policy.closest("form")!);
    fireEvent.change(environment, { target: { value: "prod" } });
    for (const value of ["disabled", "read_only", "auto"]) {
      fireEvent.change(policy, { target: { value } });
      expect(environmentForm.getByRole("status")).toHaveTextContent(labels.full);
      expect(policyForm.queryByRole("status")).toBeNull();
    }
    vi.mocked(updateWorkspaceEnvironment).mockRejectedValueOnce(new Error("disk write failed"));
    fireEvent.click(screen.getByRole("button", { name: labels.saveEnvironment }));
    await screen.findByRole("alert");
    expect(environmentForm.getByRole("status")).toHaveTextContent(labels.full);
    expect(policyForm.queryByRole("status")).toBeNull();
    expect(client.getQueryData<WorkspaceState>(["workspaces"])?.workspaces[0]).toEqual(workspace);

    vi.mocked(updateWorkspaceEnvironment).mockResolvedValue({ ...workspace, environmentType: "prod", revision: 2 });
    fireEvent.click(screen.getByRole("button", { name: labels.saveEnvironment }));
    await waitFor(() => expect(screen.getByText(labels.savedProd)).toBeTruthy());
    expect(screen.queryByRole("alert")).toBeNull();
    expect(environmentForm.getByRole("status")).toHaveTextContent(labels.full);
    expect(policyForm.getByRole("status")).toHaveTextContent(labels.auto);
    expect(policy).toHaveValue("auto");
    expect(updateWorkspaceEnvironment).toHaveBeenCalledTimes(2);
    expect(updateWorkspaceEnvironment).toHaveBeenLastCalledWith(workspace.id, "prod");
    expect(updateWorkspaceMcpPolicy).not.toHaveBeenCalled();
    expect(client.getQueryData<WorkspaceState>(["workspaces"])?.workspaces[0]).toMatchObject({ environmentType: "prod", mcpPolicy: "full_access" });
    fireEvent.change(policy, { target: { value: "disabled" } });
    expect(policyForm.queryByRole("status")).toBeNull();
    expect(environmentForm.getByRole("status")).toHaveTextContent(labels.full);
  });

  it("changes the environment risk preview only after a permission save succeeds, retaining the draft on failure", async () => {
    const workspace = { ...imported, mcpPolicy: "guarded" as const };
    const { client } = mount(workspace, locale);
    fireEvent.change(screen.getByLabelText(labels.environment), { target: { value: "prod" } });
    fireEvent.change(screen.getByLabelText(labels.policy), { target: { value: "disabled" } });
    expect(screen.getByRole("status")).toHaveTextContent(labels.guarded);
    vi.mocked(updateWorkspaceMcpPolicy).mockRejectedValueOnce(new Error("disk write failed"));
    fireEvent.click(screen.getByRole("button", { name: labels.savePolicy }));
    await screen.findByRole("alert");
    expect(screen.getByRole("status")).toHaveTextContent(labels.guarded);
    expect(screen.getByLabelText(labels.policy)).toHaveValue("disabled");
    expect(client.getQueryData<WorkspaceState>(["workspaces"])?.workspaces[0]).toEqual(workspace);

    vi.mocked(updateWorkspaceMcpPolicy).mockResolvedValue({ ...workspace, mcpPolicy: "disabled", revision: 2 });
    fireEvent.click(screen.getByRole("button", { name: labels.savePolicy }));
    await waitFor(() => expect(screen.getByText(labels.savedDisabled)).toBeTruthy());
    expect(screen.queryByRole("alert")).toBeNull();
    expect(screen.queryByRole("status")).toBeNull();
    expect(screen.getByLabelText(labels.environment)).toHaveValue("prod");
    expect(screen.getByText(labels.savedDev)).toBeTruthy();
    expect(updateWorkspaceMcpPolicy).toHaveBeenCalledTimes(2);
    expect(updateWorkspaceEnvironment).not.toHaveBeenCalled();
  });

  it("warns for a pending permission override only after the independently saved environment becomes PROD", async () => {
    mount(imported, locale);
    fireEvent.change(screen.getByLabelText(labels.environment), { target: { value: "prod" } });
    fireEvent.change(screen.getByLabelText(labels.policy), { target: { value: "guarded" } });
    expect(screen.queryByRole("status")).toBeNull();
    vi.mocked(updateWorkspaceEnvironment).mockResolvedValue({ ...imported, environmentType: "prod", revision: 2 });
    fireEvent.click(screen.getByRole("button", { name: labels.saveEnvironment }));
    await waitFor(() => expect(screen.getByText(labels.savedProd)).toBeTruthy());
    expect(screen.getByRole("status")).toHaveTextContent(labels.guarded);
    expect(screen.getByRole("status")).toHaveTextContent(labels.policyContext);
    expect(screen.getByText(labels.savedDisabled)).toBeTruthy();
    expect(screen.getByLabelText(labels.policy)).toHaveValue("guarded");
    expect(updateWorkspaceMcpPolicy).not.toHaveBeenCalled();
  });
});
