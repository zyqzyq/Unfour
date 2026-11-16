// @vitest-environment jsdom
import type { ComponentProps } from "react";
import { act, cleanup, fireEvent, render, screen, waitFor } from "@testing-library/react";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import { I18nProvider } from "@unfour/ui";
import { TemporaryTerminalWorkspace } from "./TemporaryTerminalWorkspace";

const api = vi.hoisted(() => ({ list: vi.fn(), claim: vi.fn(), start: vi.fn(), cancel: vi.fn(), close: vi.fn(), output: vi.fn() }));
vi.mock("@unfour/command-client", async (importOriginal) => ({
  ...await importOriginal<typeof import("@unfour/command-client")>(),
  listTemporarySsh: api.list, claimTemporarySsh: api.claim, startTemporarySsh: api.start,
  cancelTemporarySsh: api.cancel, closeTemporarySshWorkspace: api.close, readTemporarySshOutput: api.output,
}));
vi.mock("./TerminalPane", () => ({ TerminalPane: ({ session }: { session: { sessionId: string } }) => <div data-testid="temporary-pane">{session.sessionId}</div> }));

const first = { handle: "first", workspaceId: "workspace-a", status: "connected", expiresInSeconds: 120, fingerprint: null, errorCode: null, displayName: "Node A", endpoint: "10.0.0.8:22" };
const second = { ...first, handle: "second", displayName: "Node B", endpoint: "10.0.0.9:22" };
function view(props: Partial<ComponentProps<typeof TemporaryTerminalWorkspace>> = {}) {
  return <I18nProvider initialLocale="en"><TemporaryTerminalWorkspace workspaceId="workspace-a" {...props}><div>saved</div></TemporaryTerminalWorkspace></I18nProvider>;
}
function deferred<T>() {
  let resolve!: (value: T) => void;
  let reject!: (error: unknown) => void;
  const promise = new Promise<T>((yes, no) => { resolve = yes; reject = no; });
  return { promise, resolve, reject };
}

describe("temporary terminal operation races", () => {
  beforeEach(() => {
    vi.resetAllMocks();
    api.list.mockResolvedValue([first]); api.output.mockResolvedValue([]);
    api.claim.mockResolvedValue({ ...first, status: "claimed" });
    api.start.mockResolvedValue(undefined); api.cancel.mockResolvedValue(undefined); api.close.mockResolvedValue(undefined);
  });
  afterEach(async () => { cleanup(); await new Promise((resolve) => setTimeout(resolve, 5)); });

  it("does not resurrect a closed tab from an earlier list request", async () => {
    const stale = deferred<unknown>();
    api.list.mockResolvedValueOnce([first]).mockReturnValueOnce(stale.promise).mockResolvedValue([]);
    render(view());
    fireEvent.click(await screen.findByText("Node A · 10.0.0.8:22"));
    await waitFor(() => expect(api.list).toHaveBeenCalledTimes(2));
    fireEvent.click(screen.getByRole("button", { name: "Close / cancel" }));
    await waitFor(() => expect(screen.queryByTestId("temporary-pane")).not.toBeInTheDocument());
    await act(async () => stale.resolve([first]));
    expect(screen.queryByText("Node A · 10.0.0.8:22")).not.toBeInTheDocument();
    expect(screen.queryByRole("alert")).not.toBeInTheDocument();
    expect(api.output).toHaveBeenCalledTimes(1);
  });

  it("keeps draining other terminals when output races a closed tab", async () => {
    const output = deferred<unknown>();
    api.list.mockResolvedValue([first, second]);
    api.output.mockReturnValueOnce(output.promise);
    render(view());
    fireEvent.click(await screen.findByText("Node A · 10.0.0.8:22"));
    fireEvent.click(screen.getByRole("button", { name: "Close / cancel" }));
    await waitFor(() => expect(screen.queryByTestId("temporary-pane")).not.toBeInTheDocument());
    await act(async () => output.reject(new Error("closed while draining")));
    await waitFor(() => expect(api.output).toHaveBeenCalledWith("workspace-a", second.handle));
    expect(screen.queryByRole("alert")).not.toBeInTheDocument();
  });

  it("isolates output failures and clears the polling error after recovery", async () => {
    api.list.mockResolvedValue([first, second]);
    api.output.mockRejectedValueOnce(new Error("transient IPC failure"));
    render(view());
    await screen.findByRole("alert");
    expect(api.output).toHaveBeenCalledWith("workspace-a", second.handle);
    await waitFor(() => expect(screen.queryByRole("alert")).not.toBeInTheDocument());
    expect(api.list.mock.calls.length).toBeGreaterThan(1);
  });

  it("keeps a newly selected terminal active when an earlier close completes", async () => {
    const closed = deferred<unknown>();
    api.list.mockResolvedValue([first, second]); api.cancel.mockReturnValue(closed.promise);
    render(view());
    fireEvent.click(await screen.findByText("Node A · 10.0.0.8:22"));
    fireEvent.click(screen.getByRole("button", { name: "Close / cancel" }));
    fireEvent.click(screen.getByText("Node B · 10.0.0.9:22"));
    await act(async () => closed.resolve(undefined));
    expect(screen.getByTestId("temporary-pane")).toHaveTextContent(second.handle);
  });

  it("starts one connection for repeated clicks while claim is pending", async () => {
    const claimed = deferred<unknown>();
    api.list.mockResolvedValue([{ ...first, workspaceId: null, status: "pending" }]);
    api.claim.mockReturnValue(claimed.promise);
    render(view());
    const connect = await screen.findByRole("button", { name: "Connect in this workspace" });
    fireEvent.click(connect); fireEvent.click(connect);
    expect(api.claim).toHaveBeenCalledTimes(1);
    expect(connect).toBeDisabled();
    await act(async () => claimed.resolve({ ...first, status: "claimed" }));
    expect(api.start).toHaveBeenCalledTimes(1);
  });

  it("ignores a close failure from a workspace that has been left", async () => {
    const closed = deferred<unknown>();
    api.cancel.mockReturnValue(closed.promise);
    const mounted = render(view());
    fireEvent.click(await screen.findByText("Node A · 10.0.0.8:22"));
    fireEvent.click(screen.getByRole("button", { name: "Close / cancel" }));
    api.list.mockResolvedValue([]);
    mounted.rerender(view({ workspaceId: "workspace-b" }));
    await act(async () => closed.reject(new Error("late failure")));
    expect(screen.queryByRole("alert")).not.toBeInTheDocument();
  });

  it("does not start a claim that finishes after cancellation", async () => {
    const claimed = deferred<unknown>();
    api.list.mockResolvedValue([{ ...first, workspaceId: null, status: "pending" }]);
    api.claim.mockReturnValue(claimed.promise);
    render(view());
    fireEvent.click(await screen.findByRole("button", { name: "Connect in this workspace" }));
    fireEvent.click(screen.getByRole("button", { name: "Close / cancel" }));
    await waitFor(() => expect(screen.queryByText("Connect in this workspace")).not.toBeInTheDocument());
    api.list.mockResolvedValue([]);
    await waitFor(() => expect(api.list.mock.calls.length).toBeGreaterThan(1));
    await act(async () => claimed.resolve({ ...first, status: "claimed" }));
    expect(api.start).not.toHaveBeenCalled();
    expect(screen.queryByRole("alert")).not.toBeInTheDocument();
  });
});
