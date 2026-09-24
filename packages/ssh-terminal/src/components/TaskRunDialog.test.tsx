// @vitest-environment jsdom
import { cleanup, fireEvent, render, screen } from "@testing-library/react";
import type { SshConnection, SshTaskDetail } from "@unfour/command-client";
import { afterEach, describe, expect, it, vi } from "vitest";
import { TaskRunDialog } from "./TaskRunDialog";

vi.mock("@unfour/ui", async (importOriginal) => {
  const actual = await importOriginal<typeof import("@unfour/ui")>();
  return {
    ...actual,
    useI18n: () => ({ t: (key: string) => key }),
  };
});

afterEach(cleanup);

describe("TaskRunDialog", () => {
  it("selects a run-only variable environment and exposes load failures inline", () => {
    const onEnvironmentChange = vi.fn();
    render(
      <TaskRunDialog
        activeEnvironmentName="Dev"
        connectionId="connection"
        connections={[connection()]}
        environmentId="dev"
        environmentLoadFailed
        environments={[
          { id: "dev", name: "Dev" },
          { id: "prod", name: "Prod" },
        ]}
        error={null}
        filledFromWorkspace
        inputValues={{}}
        onConnectionChange={vi.fn()}
        onEnvironmentChange={onEnvironmentChange}
        onInputChange={vi.fn()}
        onOpenChange={vi.fn()}
        onRun={vi.fn()}
        open
        pending={false}
        secretInputNames={[]}
        task={task()}
      />,
    );

    fireEvent.change(
      screen.getByRole("combobox", { name: "ssh.tasks.run.environment" }),
      { target: { value: "prod" } },
    );
    expect(onEnvironmentChange).toHaveBeenCalledWith("prod");
    expect(screen.getByRole("alert")).toHaveTextContent(
      "ssh.tasks.run.environmentLoadFailed",
    );
  });
});

function connection(): SshConnection {
  return {
    id: "connection",
    workspaceId: "workspace",
    name: "Host",
    host: "example.com",
    port: 22,
    username: "user",
    authKind: "none",
    keyPath: null,
    credentialRef: null,
    createdAt: "2026-01-01",
    updatedAt: "2026-01-01",
    deletedAt: null,
    revision: 1,
    syncStatus: "local",
    remoteId: null,
  };
}

function task(): SshTaskDetail {
  return {
    task: {
      id: "task",
      workspaceId: "workspace",
      name: "Task",
      description: "",
      sortOrder: 0,
      createdAt: "2026-01-01",
      updatedAt: "2026-01-01",
      deletedAt: null,
    },
    steps: [],
    localBinding: null,
    detectedInputs: [],
  };
}
