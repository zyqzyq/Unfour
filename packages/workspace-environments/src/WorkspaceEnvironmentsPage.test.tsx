// @vitest-environment jsdom
import type { ReactNode } from "react";
import type {
  WorkspaceEnvironment,
  WorkspaceEnvironmentVariable,
  WorkspaceVariable,
} from "@unfour/command-client";
import { QueryClient, QueryClientProvider } from "@tanstack/react-query";
import { cleanup, fireEvent, render, screen, waitFor } from "@testing-library/react";
import { I18nProvider } from "@unfour/ui";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import { WorkspaceEnvironmentsPage } from "./WorkspaceEnvironmentsPage";

vi.mock("@unfour/command-client", () => ({
  createWorkspaceEnvironment: vi.fn(),
  previewEnvironmentImport: vi.fn(),
  importEnvironment: vi.fn(),
  exportEnvironment: vi.fn(),
  deleteWorkspaceEnvironment: vi.fn(),
  listWorkspaceEnvironments: vi.fn(),
  listWorkspaceVariables: vi.fn(),
  replaceWorkspaceVariables: vi.fn(),
  setActiveWorkspaceEnvironment: vi.fn(),
  updateWorkspaceEnvironmentVariables: vi.fn(),
}));

import {
  createWorkspaceEnvironment,
  previewEnvironmentImport,
  importEnvironment,
  exportEnvironment,
  listWorkspaceEnvironments,
  listWorkspaceVariables,
  replaceWorkspaceVariables,
  setActiveWorkspaceEnvironment,
  updateWorkspaceEnvironmentVariables,
} from "@unfour/command-client";

const listEnvironmentsMock = vi.mocked(listWorkspaceEnvironments);
const listVariablesMock = vi.mocked(listWorkspaceVariables);
const createMock = vi.mocked(createWorkspaceEnvironment);
const updateMock = vi.mocked(updateWorkspaceEnvironmentVariables);
const replaceMock = vi.mocked(replaceWorkspaceVariables);
const activateMock = vi.mocked(setActiveWorkspaceEnvironment);

function environmentVariable(
  overrides: Partial<WorkspaceEnvironmentVariable> = {},
): WorkspaceEnvironmentVariable {
  return {
    id: "var-1",
    workspaceId: "ws-1",
    environmentId: "env-1",
    key: "base_url",
    value: "https://local.example.com",
    isSecret: false,
    isEnabled: true,
    description: null,
    sortOrder: 0,
    createdAt: "2026-01-01T00:00:00Z",
    updatedAt: "2026-01-01T00:00:00Z",
    deletedAt: null,
    revision: 1,
    ...overrides,
  };
}

function environment(overrides: Partial<WorkspaceEnvironment> = {}): WorkspaceEnvironment {
  return {
    id: "env-1",
    workspaceId: "ws-1",
    name: "Local",
    sortOrder: 0,
    variables: [],
    isActive: false,
    createdAt: "2026-01-01T00:00:00Z",
    updatedAt: "2026-01-01T00:00:00Z",
    deletedAt: null,
    revision: 1,
    ...overrides,
  };
}

function workspaceVariable(overrides: Partial<WorkspaceVariable> = {}): WorkspaceVariable {
  const variable = environmentVariable(overrides);
  const { environmentId: _environmentId, ...workspaceVariable } = variable;
  return workspaceVariable;
}

function createWrapper() {
  const client = new QueryClient({
    defaultOptions: { mutations: { retry: false }, queries: { retry: false } },
  });
  return function Wrapper({ children }: { children: ReactNode }) {
    return (
      <I18nProvider initialLocale="en">
        <QueryClientProvider client={client}>{children}</QueryClientProvider>
      </I18nProvider>
    );
  };
}

function renderPage(
  initialEnvironmentId: string | null = "env-1",
  variableDecoration?: (
    variable: WorkspaceVariable | WorkspaceEnvironmentVariable,
  ) => ReactNode,
) {
  render(
    <WorkspaceEnvironmentsPage
      initialEnvironmentId={initialEnvironmentId}
      onClose={vi.fn()}
      variableDecoration={variableDecoration}
      workspaceId="ws-1"
    />,
    { wrapper: createWrapper() },
  );
}

beforeEach(() => {
  vi.clearAllMocks();
  listEnvironmentsMock.mockResolvedValue([
    environment({ id: "env-1", name: "Local", isActive: true }),
    environment({ id: "env-2", name: "Test" }),
  ]);
  listVariablesMock.mockResolvedValue([]);
  createMock.mockResolvedValue(environment({ id: "env-3", name: "QA" }));
  updateMock.mockResolvedValue(environment({ id: "env-3", name: "QA" }));
  replaceMock.mockResolvedValue([
    workspaceVariable({ key: "base_url", value: "https://workspace.example.com" }),
  ]);
});

afterEach(() => {
  cleanup();
  vi.resetAllMocks();
});

describe("WorkspaceEnvironmentsPage", () => {
  it("previews a same-name environment and imports only after confirmation", async () => {
    vi.mocked(previewEnvironmentImport).mockResolvedValue({content:"environment-content",preview:{format:"postman",name:"Local",variables:[{key:"credential",isSecret:true,isEnabled:false}],conflict:true,warnings:["environmentCopy"]}});
    vi.mocked(importEnvironment).mockResolvedValue(environment({id:"copy",name:"Local (Copy 1)"}));
    renderPage();
    fireEvent.click(await screen.findByRole("button",{name:"Import",exact:true}));
    await screen.findByText("An environment with this name exists. A copy will be created.");
    expect(importEnvironment).not.toHaveBeenCalled();
    fireEvent.click(screen.getAllByRole("button",{name:"Import",exact:true}).at(-1)!);
    await waitFor(() => expect(importEnvironment).toHaveBeenCalledWith("ws-1","environment-content"));
    expect(updateMock).not.toHaveBeenCalled();
    expect(replaceMock).not.toHaveBeenCalled();
  });

  it("exports an environment from its menu with a format choice", async () => {
    vi.mocked(exportEnvironment).mockResolvedValue({saved:true});
    renderPage();
    fireEvent.pointerDown(await screen.findByRole("button",{name:"Variable environment actions for Local"}));
    fireEvent.click(await screen.findByRole("menuitem",{name:"Export",exact:true}));
    fireEvent.change(await screen.findByRole("combobox",{name:"Format"}),{target:{value:"postman"}});
    fireEvent.click(screen.getByRole("button",{name:"Export",exact:true}));
    await waitFor(() => expect(exportEnvironment).toHaveBeenCalledWith("ws-1","env-1","postman"));
  });

  it("edits the fixed workspace variable collection", async () => {
    renderPage(null);

    expect(
      await screen.findByRole("heading", { name: "Workspace Variables" }),
    ).toBeTruthy();
    expect(screen.getByText("Variable environments")).toBeTruthy();
    fireEvent.click(screen.getByRole("button", { name: "Add variable" }));
    fireEvent.change(screen.getByPlaceholderText("Key"), {
      target: { value: "base_url" },
    });
    fireEvent.change(screen.getByPlaceholderText("Value"), {
      target: { value: "https://workspace.example.com" },
    });
    fireEvent.click(screen.getByRole("button", { name: "Save" }));

    await waitFor(() =>
      expect(replaceMock).toHaveBeenCalledWith("ws-1", [
        {
          key: "base_url",
          value: "https://workspace.example.com",
          isSecret: false,
          isEnabled: true,
          description: null,
          sortOrder: 0,
        },
      ]),
    );
  });

  it("selecting an environment for editing does not activate it", async () => {
    renderPage();
    await screen.findByRole("heading", { name: "Local" });

    fireEvent.click(screen.getByRole("button", { name: "Test" }));

    expect(await screen.findByRole("heading", { name: "Test" })).toBeTruthy();
    expect(activateMock).not.toHaveBeenCalled();
  });

  it("creates an environment from the workspace manager", async () => {
    renderPage();
    await screen.findByRole("heading", { name: "Local" });
    fireEvent.click(screen.getByRole("button", { name: "New" }));
    fireEvent.change(screen.getByLabelText("Name"), { target: { value: "QA" } });
    fireEvent.click(screen.getByRole("button", { name: "Save" }));

    await waitFor(() => expect(createMock).toHaveBeenCalledWith("ws-1", "QA"));
  });

  it("decorates persisted workspace and environment variables", async () => {
    const environmentValue = environmentVariable({ id: "env-var" });
    listEnvironmentsMock.mockResolvedValue([
      environment({ isActive: true, variables: [environmentValue] }),
    ]);
    listVariablesMock.mockResolvedValue([workspaceVariable({ id: "workspace-var" })]);
    renderPage("env-1", (variable) => (
      <span>{`decor-${"environmentId" in variable ? "environment" : "workspace"}-${variable.id}`}</span>
    ));

    expect(await screen.findByText("decor-environment-env-var")).toBeTruthy();
    fireEvent.click(screen.getByRole("button", { name: "Workspace Variables" }));
    expect(await screen.findByText("decor-workspace-workspace-var")).toBeTruthy();
  });
});
