// @vitest-environment jsdom
import { cleanup, fireEvent, render, screen } from "@testing-library/react";
import { afterEach, describe, expect, it, vi } from "vitest";
import type {
  ButtonHTMLAttributes,
  InputHTMLAttributes,
  ReactNode,
  SelectHTMLAttributes,
} from "react";
import type { SshTaskSaveInput } from "@unfour/command-client";
import { TaskEditor } from "./TaskEditor";

vi.mock("@unfour/ui", () => ({
  Badge: ({ children }: { children: ReactNode }) => <span>{children}</span>,
  Button: ({ children, ...props }: ButtonHTMLAttributes<HTMLButtonElement>) => (
    <button {...props}>{children}</button>
  ),
  Input: ({ className, ...props }: InputHTMLAttributes<HTMLInputElement>) => (
    <input className={className} {...props} />
  ),
  Select: ({ children, ...props }: SelectHTMLAttributes<HTMLSelectElement>) => (
    <select {...props}>{children}</select>
  ),
  useI18n: () => ({ t: (key: string) => key }),
}));

vi.mock("./TaskEditorSteps", () => ({
  AddStepMenu: () => null,
  StepInsertSlot: () => null,
  StepRow: () => null,
}));

afterEach(cleanup);

describe("TaskEditor title field", () => {
  it("prevents draft changes while the submitted version is being saved", () => {
    render(<TaskEditor connections={[]} draft={draft({ name: "Pending" })} onChange={vi.fn()} onRun={vi.fn()} onSave={vi.fn()} saving />);
    expect(screen.getByRole("textbox", { name: "ssh.tasks.editor.name" })).toBeDisabled();
    expect(screen.getByRole("combobox", { name: "ssh.tasks.editor.defaultConnection" })).toBeDisabled();
  });
  it("exposes the complete long title while giving the editor flexible space", () => {
    const name = "Deploy the production service with the latest migration bundle";
    render(
      <TaskEditor
        connections={[]}
        draft={draft({ name })}
        onChange={vi.fn()}
        onRun={vi.fn()}
        onSave={vi.fn()}
        saving={false}
      />,
    );

    const input = screen.getByRole("textbox", { name: "ssh.tasks.editor.name" });

    expect(input).toHaveValue(name);
    expect(input).toHaveAttribute("title", name);
    expect(input.className).toContain("flex-[1_1_320px]");
    expect(input.className).toContain("max-w-[420px]");
  });

  it("keeps title changes in the task draft", () => {
    const onChange = vi.fn();
    render(
      <TaskEditor
        connections={[]}
        draft={draft({ name: "Old title" })}
        onChange={onChange}
        onRun={vi.fn()}
        onSave={vi.fn()}
        saving={false}
      />,
    );

    fireEvent.change(screen.getByRole("textbox", { name: "ssh.tasks.editor.name" }), {
      target: { value: "Updated title" },
    });

    expect(onChange).toHaveBeenCalledWith(expect.objectContaining({ name: "Updated title" }));
  });
});

function draft(overrides: Partial<SshTaskSaveInput> = {}): SshTaskSaveInput {
  return {
    workspaceId: "workspace-one",
    name: "Task",
    description: "",
    defaultConnectionId: null,
    steps: [],
    ...overrides,
  };
}
