// @vitest-environment jsdom
import type { ReactNode } from "react";
import type { ApiEnvironment } from "@unfour/command-client";
import { QueryClient, QueryClientProvider } from "@tanstack/react-query";
import { cleanup, fireEvent, render, screen, waitFor } from "@testing-library/react";
import { I18nProvider } from "@unfour/ui";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import { EnvironmentManagerPage } from "./EnvironmentManagerPage";

vi.mock("@unfour/command-client", () => ({
  activateApiEnvironment: vi.fn(),
  createApiEnvironment: vi.fn(),
  deleteApiEnvironment: vi.fn(),
  listApiEnvironments: vi.fn(),
  updateApiEnvironment: vi.fn(),
}));

import {
  createApiEnvironment,
  deleteApiEnvironment,
  listApiEnvironments,
  updateApiEnvironment,
} from "@unfour/command-client";

const listMock = vi.mocked(listApiEnvironments);
const createMock = vi.mocked(createApiEnvironment);
const updateMock = vi.mocked(updateApiEnvironment);
const deleteMock = vi.mocked(deleteApiEnvironment);

function environment(overrides: Partial<ApiEnvironment> = {}): ApiEnvironment {
  return {
    id: "env-1",
    workspaceId: "ws-1",
    name: "Local",
    variables: [],
    isActive: false,
    createdAt: "2026-01-01T00:00:00Z",
    updatedAt: "2026-01-01T00:00:00Z",
    ...overrides,
  };
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

function renderManager(
  initialMode: React.ComponentProps<typeof EnvironmentManagerPage>["initialMode"] = {
    kind: "manage",
    nonce: 1,
  },
) {
  const onDirtyChange = vi.fn();
  render(
    <EnvironmentManagerPage
      initialMode={initialMode}
      onDirtyChange={onDirtyChange}
      workspaceId="ws-1"
    />,
    { wrapper: createWrapper() },
  );
  return { onDirtyChange };
}

beforeEach(() => {
  vi.clearAllMocks();
  listMock.mockResolvedValue([environment()]);
  createMock.mockResolvedValue(environment({ id: "env-2", name: "QA" }));
  updateMock.mockResolvedValue(
    environment({
      id: "env-2",
      name: "QA",
      variables: [{ enabled: true, key: "base_url", value: "https://qa.example.com" }],
    }),
  );
  deleteMock.mockResolvedValue([]);
});

afterEach(() => {
  cleanup();
  vi.resetAllMocks();
});

describe("EnvironmentManagerPage", () => {
  it("creates a new environment and then saves its variables", async () => {
    listMock.mockResolvedValue([]);
    renderManager({ kind: "new", nonce: 1 });

    fireEvent.change(await screen.findByLabelText("Name"), {
      target: { value: "QA" },
    });
    fireEvent.change(screen.getAllByPlaceholderText("Key")[0], {
      target: { value: "base_url" },
    });
    fireEvent.change(screen.getAllByPlaceholderText("Value")[0], {
      target: { value: "https://qa.example.com" },
    });
    fireEvent.click(screen.getByRole("button", { name: "Save" }));

    await waitFor(() => expect(createMock).toHaveBeenCalledWith("ws-1", "QA"));
    await waitFor(() =>
      expect(updateMock).toHaveBeenCalledWith("ws-1", "env-2", "QA", [
        { enabled: true, key: "base_url", value: "https://qa.example.com" },
      ]),
    );
  });

  it("blocks duplicate environment names before save", async () => {
    renderManager({ kind: "new", nonce: 1 });

    fireEvent.change(await screen.findByLabelText("Name"), {
      target: { value: "local" },
    });

    expect(
      screen.getByText("An environment named local already exists in this workspace."),
    ).toBeInTheDocument();
    expect(screen.getByRole("button", { name: "Save" })).toBeDisabled();
  });

  it("confirms before deleting an environment from the actions menu", async () => {
    renderManager({ kind: "edit", environmentId: "env-1", nonce: 1 });

    expect(screen.queryByRole("button", { name: "Delete environment" })).toBeNull();

    fireEvent.click(await screen.findByRole("button", { name: "Environment actions" }));
    fireEvent.click(await screen.findByRole("menuitem", { name: "Delete environment" }));
    expect(
      screen.getByText('Delete environment "Local"? This cannot be undone.'),
    ).toBeInTheDocument();

    fireEvent.click(screen.getByRole("button", { name: "Delete environment" }));

    await waitFor(() => expect(deleteMock).toHaveBeenCalledWith("ws-1", "env-1"));
  });
});
