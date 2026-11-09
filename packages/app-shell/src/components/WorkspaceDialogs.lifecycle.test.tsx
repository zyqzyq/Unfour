// @vitest-environment jsdom
import { useState } from "react";
import { QueryClient, QueryClientProvider } from "@tanstack/react-query";
import { act, cleanup, fireEvent, render, screen, waitFor } from "@testing-library/react";
import { afterEach, beforeEach, expect, it, vi } from "vitest";
import { I18nProvider } from "@unfour/ui";
import { createWorkspace, deleteWorkspace, type Workspace } from "@unfour/command-client";
import { WorkspaceDialogs } from "./WorkspaceDialogs";
import { WorkspaceLeaveContext } from "./workspace-leave-context";

const { handleError, setActiveWorkspace } = vi.hoisted(() => ({ handleError: vi.fn(), setActiveWorkspace: vi.fn() }));
vi.mock("@unfour/ui", async (original) => ({ ...await original<typeof import("@unfour/ui")>(), useFeedbackErrorHandler: () => handleError }));
vi.mock("@unfour/workspace-core", () => ({ useWorkspaceStore: () => ({ setActiveWorkspace }) }));
vi.mock("@unfour/workspace-local", () => ({ WorkspaceSecurityDialog: () => null }));
vi.mock("@unfour/command-client", () => ({ createWorkspace: vi.fn(), deleteWorkspace: vi.fn(), renameWorkspace: vi.fn() }));

beforeEach(() => vi.resetAllMocks());
afterEach(cleanup);

const workspace: Workspace = { id: "w", name: "Original", environmentType: "test", mcpPolicy: "auto", isDefault: false, lastOpenedAt: null, createdAt: "t", updatedAt: "t", deletedAt: null, revision: 1 };
const fallback = { ...workspace, id: "default", name: "Default", isDefault: true };
type Action = "create" | "delete";

function deferred<T>() {
  let resolve!: (value: T) => void;
  let reject!: (error: Error) => void;
  const promise = new Promise<T>((res, rej) => { resolve = res; reject = rej; });
  return { promise, resolve, reject };
}

function mount(action: Action, leave = vi.fn((run: () => void) => run())) {
  const client = new QueryClient({ defaultOptions: { mutations: { retry: false } } });
  const close = vi.fn();
  function Host() {
    const [open, setOpen] = useState(true);
    const onClose = () => { close(); setOpen(false); };
    return <I18nProvider initialLocale="en"><QueryClientProvider client={client}>
      <WorkspaceLeaveContext.Provider value={leave}>
        <WorkspaceDialogs activeWorkspace={workspace} workspaces={[workspace, fallback]}
          createOpen={action === "create" && open} deleteOpen={action === "delete" && open} environmentOpen={false} renameOpen={false}
          onCreateClose={onClose} onDeleteClose={onClose} onEnvironmentClose={vi.fn()} onRenameClose={vi.fn()} />
      </WorkspaceLeaveContext.Provider>
    </QueryClientProvider></I18nProvider>;
  }
  render(<Host />);
  if (action === "create") {
    fireEvent.change(screen.getByRole("textbox"), { target: { value: "New fixture" } });
    fireEvent.change(screen.getByRole("combobox"), { target: { value: "test" } });
  }
  const submit = screen.getByRole("button", { name: action === "create" ? "Create" : "Delete" });
  const command = action === "create" ? createWorkspace : deleteWorkspace;
  return { close, command, leave, submit };
}

for (const action of ["create", "delete"] as const) {
  it(`allows ${action} overlay dismissal before submitting`, async () => {
    const { close, command } = mount(action);
    await act(async () => { await new Promise((resolve) => setTimeout(resolve, 0)); });
    const overlay = screen.getByRole("dialog").previousElementSibling!;
    fireEvent.pointerDown(overlay, { pointerType: "mouse", button: 0 });
    fireEvent.click(overlay);
    expect(close).toHaveBeenCalledOnce();
    expect(command).not.toHaveBeenCalled();
  });

  for (const dismissal of ["cancel", "escape", "overlay", "close callback"] as const) {
    it(`blocks ${action} ${dismissal} during a deferred command`, async () => {
      vi.mocked(createWorkspace).mockReturnValue(new Promise(() => {}));
      vi.mocked(deleteWorkspace).mockReturnValue(new Promise(() => {}));
      const { close, command, submit } = mount(action);
      fireEvent.click(submit);
      await waitFor(() => expect(command).toHaveBeenCalledOnce());
      await waitFor(() => expect(submit).toBeDisabled());
      const cancel = screen.getByRole("button", { name: "Cancel" });
      if (dismissal === "cancel") {
        fireEvent.click(cancel);
        expect(cancel).toBeDisabled();
      } else if (dismissal === "escape") {
        fireEvent.keyDown(document, { key: "Escape" });
      } else if (dismissal === "overlay") {
        // Radix installs the outside-pointer listener on the next timer tick.
        await act(async () => { await new Promise((resolve) => setTimeout(resolve, 0)); });
        const overlay = screen.getByRole("dialog").previousElementSibling!;
        fireEvent.pointerDown(overlay, { pointerType: "mouse", button: 0 });
        fireEvent.click(overlay);
      } else {
        // Exercise Dialog.Close -> onOpenChange independently of button.disabled.
        cancel.removeAttribute("disabled");
        fireEvent.click(cancel);
      }
      expect(close).not.toHaveBeenCalled();
      expect(screen.getByRole("dialog")).toBeVisible();
      expect(setActiveWorkspace).not.toHaveBeenCalled();
      if (action === "create") {
        expect(screen.getByRole("textbox")).toBeDisabled();
        expect(screen.getByRole("combobox")).toBeDisabled();
      }
    });
  }

  it(`deduplicates ${action} submissions before and after the pending render`, async () => {
    vi.mocked(createWorkspace).mockReturnValue(new Promise(() => {}));
    vi.mocked(deleteWorkspace).mockReturnValue(new Promise(() => {}));
    const { command, submit, leave } = mount(action);
    const trigger = () => action === "create" ? fireEvent.submit(submit.closest("form")!) : fireEvent.click(submit);
    act(() => { trigger(); trigger(); trigger(); });
    await waitFor(() => expect(command).toHaveBeenCalledOnce());
    trigger();
    await act(async () => {});
    expect(command).toHaveBeenCalledOnce();
    expect(leave).toHaveBeenCalledOnce();
  });

  it(`keeps ${action} retryable after failure and switches once on success`, async () => {
    const creation = deferred<Workspace>();
    const deletion = deferred<Awaited<ReturnType<typeof deleteWorkspace>>>();
    vi.mocked(createWorkspace).mockReturnValue(creation.promise);
    vi.mocked(deleteWorkspace).mockReturnValue(deletion.promise);
    const { command, close, submit } = mount(action);
    fireEvent.click(submit);
    await waitFor(() => expect(command).toHaveBeenCalledOnce());
    const error = new Error("Isolated failure");
    await act(async () => { (action === "create" ? creation : deletion).reject(error); });
    await waitFor(() => expect(submit).toBeEnabled());
    expect(handleError).toHaveBeenCalledWith(error, { key: `feedback.workspace.${action}Failed` });
    expect(close).not.toHaveBeenCalled();
    expect(setActiveWorkspace).not.toHaveBeenCalled();
    expect(screen.getByRole("button", { name: "Cancel" })).toBeEnabled();
    if (action === "create") {
      expect(screen.getByRole("textbox")).toHaveValue("New fixture");
      expect(screen.getByRole("textbox")).toBeEnabled();
      expect(screen.getByRole("combobox")).toHaveValue("test");
      expect(screen.getByRole("combobox")).toBeEnabled();
    }
    vi.mocked(createWorkspace).mockResolvedValue({ ...workspace, id: "new" });
    vi.mocked(deleteWorkspace).mockResolvedValue({ workspaces: [fallback], activeWorkspaceId: fallback.id });
    fireEvent.click(submit);
    await waitFor(() => expect(close).toHaveBeenCalledOnce());
    expect(command).toHaveBeenCalledTimes(2);
    expect(setActiveWorkspace).toHaveBeenCalledExactlyOnceWith(action === "create" ? "new" : fallback.id);
    expect(screen.queryByRole("dialog")).not.toBeInTheDocument();
    if (action === "create") expect(createWorkspace).toHaveBeenLastCalledWith("New fixture", "test");
  });

  it(`does not lock ${action} when the leave confirmation is cancelled`, async () => {
    const leave = vi.fn((_run: () => void) => {});
    const { command, close, submit } = mount(action, leave);
    fireEvent.click(submit);
    await act(async () => {});
    expect(leave).toHaveBeenCalledOnce();
    expect(command).not.toHaveBeenCalled();
    fireEvent.click(screen.getByRole("button", { name: "Cancel" }));
    expect(close).toHaveBeenCalledOnce();
    expect(setActiveWorkspace).not.toHaveBeenCalled();
  });

  it(`deduplicates ${action} callbacks from the deferred leave gate`, async () => {
    vi.mocked(createWorkspace).mockReturnValue(new Promise(() => {}));
    vi.mocked(deleteWorkspace).mockReturnValue(new Promise(() => {}));
    const leave = vi.fn((_run: () => void) => {});
    const { command, submit } = mount(action, leave);
    fireEvent.click(submit);
    const run = leave.mock.calls[0][0];
    await act(async () => { run(); run(); });
    expect(command).toHaveBeenCalledOnce();
  });
}
