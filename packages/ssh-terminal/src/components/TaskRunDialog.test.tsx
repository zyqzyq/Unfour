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
  it.each(["connection", "environment", "inputs", "pending"])("blocks execution with missing %s and keeps recall explicit", (missing) => {
    const onRun = vi.fn();
    const onUsePrevious = vi.fn();
    const detail = task();
    detail.steps = [{ id: "step", workspaceId: "workspace", taskId: "task", name: "Command", stepType: "command", position: 0, enabled: true, configVersion: 1,
      configJson: { command: "echo {{VERSION}}" }, createdAt: "", updatedAt: "", deletedAt: null }];
    render(<TaskRunDialog activeEnvironmentName={null} connectionId="connection" connections={missing === "connection" ? [] : [connection()]}
      environmentId={missing === "environment" ? "deleted" : ""} environmentLoadFailed={false} environments={[]} error={null}
      filledFromWorkspace={false} inputValues={{ VERSION: missing === "inputs" ? "" : "v1" }} onConnectionChange={vi.fn()} onEnvironmentChange={vi.fn()}
      onInputChange={vi.fn()} onOpenChange={vi.fn()} onRun={onRun} onUsePrevious={onUsePrevious} open pending={missing === "pending"} secretInputNames={[]} task={detail} />);
    expect(screen.getByRole("button", { name: missing === "pending" ? "ssh.tasks.run.starting" : "ssh.tasks.actions.run" })).toBeDisabled();
    fireEvent.click(screen.getByRole("button", { name: "ssh.tasks.run.usePrevious" }));
    expect(onRun).not.toHaveBeenCalled();
    expect(onUsePrevious).toHaveBeenCalledTimes(missing === "pending" ? 0 : 1);
  });
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
        error={new Error("SSH_TASK_INPUTS_CHANGED")}
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
    expect(screen.getAllByRole("alert").map((node) => node.textContent)).toEqual(["ssh.tasks.run.environmentLoadFailed", "ssh.tasks.run.inputsChanged"]);
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
