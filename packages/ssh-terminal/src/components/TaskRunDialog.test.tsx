// @vitest-environment jsdom
import { cleanup, fireEvent, render, screen } from "@testing-library/react";
import { useState, type ComponentProps } from "react";
import type { SshConnection, SshTaskDetail } from "@unfour/command-client";
import { createTranslator, I18nProvider } from "@unfour/ui";
import { afterEach, describe, expect, it, vi } from "vitest";
import { TaskRunDialog } from "./TaskRunDialog";

const t = createTranslator("en");

afterEach(cleanup);

describe("TaskRunDialog", () => {
  it.each(["connection", "environment", "inputs", "pending"])("blocks execution with missing %s and keeps recall explicit", (missing) => {
    const onRun = vi.fn();
    const onUsePrevious = vi.fn();
    const detail = task();
    detail.steps = [{ id: "step", workspaceId: "workspace", taskId: "task", name: "Command", stepType: "command", position: 0, enabled: true, configVersion: 1,
      configJson: { command: "echo {{VERSION}}" }, createdAt: "", updatedAt: "", deletedAt: null }];
    render(<TaskRunDialog activeEnvironmentName={null} automaticSecretInputNames={[]} inputScopeKey={0} connectionId="connection" connections={missing === "connection" ? [] : [connection()]}
      environmentId={missing === "environment" ? "deleted" : ""} environmentLoadFailed={false} environments={[]} error={null}
      filledFromWorkspace={false} inputValues={{ VERSION: missing === "inputs" ? "" : "v1" }} onConnectionChange={vi.fn()} onEnvironmentChange={vi.fn()}
      onInputChange={vi.fn()} onOpenChange={vi.fn()} onRun={onRun} onUsePrevious={onUsePrevious} open pending={missing === "pending"} secretInputNames={[]} task={detail} />);
    expect(screen.getByRole("button", { name: t(missing === "pending" ? "ssh.tasks.run.starting" : "ssh.tasks.actions.run") })).toBeDisabled();
    fireEvent.click(screen.getByRole("button", { name: t("ssh.tasks.run.usePrevious") }));
    expect(onRun).not.toHaveBeenCalled();
    expect(onUsePrevious).toHaveBeenCalledTimes(missing === "pending" ? 0 : 1);
  });
  it("selects a run-only variable environment and exposes load failures inline", () => {
    const onEnvironmentChange = vi.fn();
    render(
      <TaskRunDialog
        activeEnvironmentName="Dev"
        automaticSecretInputNames={[]}
        inputScopeKey={0}
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
      screen.getByRole("combobox", { name: t("ssh.tasks.run.environment") }),
      { target: { value: "prod" } },
    );
    expect(onEnvironmentChange).toHaveBeenCalledWith("prod");
    expect(screen.getAllByRole("alert").map((node) => node.textContent)).toEqual([t("ssh.tasks.run.environmentLoadFailed"), t("ssh.tasks.run.inputsChanged")]);
  });

  it("toggles manual sensitivity beside the label and keeps the input value", () => {
    const props = inputProps();
    render(<EditableDialog {...props} />);
    const lock = screen.getByRole("button", { name: `VERSION: ${t("ssh.tasks.run.markSensitive")}` });
    expect(lock).toHaveAttribute("aria-pressed", "false");
    expect(lock.parentElement).toContainElement(screen.getByText("VERSION", { selector: "label" }));
    expect(screen.getByLabelText("VERSION")).toHaveAttribute("type", "text");
    fireEvent.click(lock);
    expect(screen.getByLabelText("VERSION")).toHaveAttribute("type", "password");
    expect(lock).toHaveAttribute("aria-pressed", "true");
    fireEvent.click(screen.getByRole("button", { name: `VERSION: ${t("ssh.tasks.run.unmarkSensitive")}` }));
    expect(screen.getByLabelText("VERSION")).toHaveAttribute("type", "text");
    expect(screen.getByLabelText("VERSION")).toHaveValue("v1");
    expect(props.onToggleSecret).toHaveBeenNthCalledWith(1, "VERSION");
    expect(props.onToggleSecret).toHaveBeenNthCalledWith(2, "VERSION");
    expect(props.onInputChange).not.toHaveBeenCalled();
  });

  it.each(["PASSWORD", "api_token", "CUSTOM"])("keeps automatic %s locked with a focusable explanation", (name) => {
    const props = inputProps(name);
    render(<TaskRunDialog {...props} automaticSecretInputNames={[name]} secretInputNames={[name]} />);
    const lock = screen.getByRole("button", { name: `${name}: ${t("ssh.tasks.run.secretInput")}` });
    expect(lock).toHaveAttribute("aria-pressed", "true");
    expect(lock).toHaveAttribute("aria-disabled", "true");
    expect(lock).toHaveAccessibleDescription(t("ssh.tasks.run.automaticSensitive"));
    expect(lock).toHaveAttribute("title", t("ssh.tasks.run.automaticSensitive"));
    lock.focus();
    expect(lock).toHaveFocus();
    fireEvent.click(lock);
    expect(props.onToggleSecret).not.toHaveBeenCalled();
    expect(screen.getByLabelText(name)).toHaveAttribute("type", "password");
  });

  it.each(["en", "zh-CN"] as const)("changes visibility independently with %s labels", (locale) => {
    const translate = createTranslator(locale);
    const props = { ...inputProps(), secretInputNames: ["VERSION"] };
    render(<I18nProvider initialLocale={locale}><TaskRunDialog {...props} /></I18nProvider>);
    expect(screen.getByLabelText("VERSION")).toHaveAttribute("type", "password");
    fireEvent.click(screen.getByRole("button", { name: translate("variables.showSecret") }));
    expect(screen.getByLabelText("VERSION")).toHaveAttribute("type", "text");
    expect(screen.getByRole("button", { name: `VERSION: ${translate("ssh.tasks.run.unmarkSensitive")}` })).toHaveAttribute("aria-pressed", "true");
    fireEvent.click(screen.getByRole("button", { name: translate("variables.hideSecret") }));
    expect(screen.getByLabelText("VERSION")).toHaveAttribute("type", "password");
    expect(screen.getByLabelText("VERSION")).toHaveValue("v1");
    expect(props.onInputChange).not.toHaveBeenCalled();
    expect(props.onToggleSecret).not.toHaveBeenCalled();
  });

  it("hides a revealed value when a new parameter scope is prepared", () => {
    const props = { ...inputProps(), secretInputNames: ["VERSION"] };
    const { rerender } = render(<TaskRunDialog {...props} />);
    fireEvent.click(screen.getByRole("button", { name: t("variables.showSecret") }));
    expect(screen.getByLabelText("VERSION")).toHaveAttribute("type", "text");
    rerender(<TaskRunDialog {...props} inputScopeKey={1} inputValues={{ VERSION: "resolved-again" }} />);
    expect(screen.getByLabelText("VERSION")).toHaveAttribute("type", "password");
    expect(screen.getByLabelText("VERSION")).toHaveValue("resolved-again");
  });

  it("disables sensitivity and visibility controls while preparing or starting", () => {
    render(<TaskRunDialog {...inputProps()} secretInputNames={["VERSION"]} pending />);
    expect(screen.getByRole("button", { name: `VERSION: ${t("ssh.tasks.run.unmarkSensitive")}` })).toBeDisabled();
    expect(screen.getByRole("button", { name: t("variables.showSecret") })).toBeDisabled();
    expect(screen.getByLabelText("VERSION")).toBeDisabled();
  });
});

type DialogProps = ComponentProps<typeof TaskRunDialog>;

function inputProps(name = "VERSION"): DialogProps {
  const detail = task();
  detail.steps = [{ id: "step", workspaceId: "workspace", taskId: "task", name: "Command", stepType: "command", position: 0, enabled: true, configVersion: 1,
    configJson: { command: `echo {{${name}}}` }, createdAt: "", updatedAt: "", deletedAt: null }];
  return { activeEnvironmentName: null, automaticSecretInputNames: [], inputScopeKey: 0,
    connectionId: "connection", connections: [connection()], environmentId: "", environmentLoadFailed: false, environments: [], error: null,
    filledFromWorkspace: false, inputValues: { [name]: "v1" }, onConnectionChange: vi.fn(), onEnvironmentChange: vi.fn(), onInputChange: vi.fn(),
    onOpenChange: vi.fn(), onRun: vi.fn(), onToggleSecret: vi.fn(), open: true, pending: false, secretInputNames: [], task: detail };
}

function EditableDialog(props: DialogProps) {
  const [secretInputNames, setSecretInputNames] = useState(props.secretInputNames);
  return <TaskRunDialog {...props} secretInputNames={secretInputNames} onToggleSecret={(name) => {
    props.onToggleSecret?.(name);
    setSecretInputNames((names) => names.includes(name) ? names.filter((key) => key !== name) : [...names, name]);
  }} />;
}

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
