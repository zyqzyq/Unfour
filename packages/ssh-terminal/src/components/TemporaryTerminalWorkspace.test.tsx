// @vitest-environment jsdom
import { StrictMode } from "react";
import { act, cleanup, fireEvent, render, screen, waitFor } from "@testing-library/react";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import { I18nProvider, createTranslator } from "@unfour/ui";
import { TemporaryTerminalWorkspace } from "./TemporaryTerminalWorkspace";

const api = vi.hoisted(() => ({ list: vi.fn(), claim: vi.fn(), start: vi.fn(), cancel: vi.fn(), close: vi.fn(), output: vi.fn(), trust: vi.fn() }));
vi.mock("@unfour/command-client", async (importOriginal) => ({
  ...await importOriginal<typeof import("@unfour/command-client")>(),
  listTemporarySsh: api.list, claimTemporarySsh: api.claim, startTemporarySsh: api.start,
  cancelTemporarySsh: api.cancel, closeTemporarySshWorkspace: api.close, readTemporarySshOutput: api.output, trustTemporarySsh: api.trust,
}));
vi.mock("./TerminalPane", () => ({ TerminalPane: ({ temporary, inputDisabled }: { temporary: boolean; inputDisabled: boolean }) => <div data-testid="temporary-pane" data-input-disabled={inputDisabled}>{String(temporary)}</div> }));
const pending = { handle: "opaque-fixture", workspaceId: null, status: "pending", expiresInSeconds: 120, fingerprint: null, errorCode: null, displayName: "Production node", endpoint: "10.0.0.8:22" };
function view(workspaceId = "workspace-a", autoClaim = false, surfaceActive = true, locale: "en" | "zh-CN" = "en") {
  return <I18nProvider initialLocale={locale}><TemporaryTerminalWorkspace workspaceId={workspaceId} autoClaim={autoClaim} surfaceActive={surfaceActive}><div data-testid="saved">saved</div></TemporaryTerminalWorkspace></I18nProvider>;
}
describe("temporary terminal lifecycle", () => {
  afterEach(async () => { cleanup(); await new Promise((resolve) => setTimeout(resolve, 5)); });
  beforeEach(() => {
    vi.clearAllMocks();
    api.list.mockResolvedValue([pending]); api.output.mockResolvedValue([]);
    api.claim.mockResolvedValue({ ...pending, workspaceId: "workspace-a", status: "claimed" });
    api.start.mockResolvedValue(undefined); api.cancel.mockResolvedValue(undefined); api.close.mockResolvedValue(undefined); api.trust.mockResolvedValue(undefined);
  });
  it.each(["terminalTaskBusy", "terminalRecoveryRequired"])("disables manual terminal input for %s", async (field) => {
    api.list.mockResolvedValue([{ ...pending, workspaceId: "workspace-a", status: "connected", [field]: true }]);
    render(view());
    fireEvent.click(await screen.findByText("Production node · 10.0.0.8:22"));
    expect(screen.getByTestId("temporary-pane")).toHaveAttribute("data-input-disabled", "true");
    if (field === "terminalTaskBusy") expect(screen.getByText("Terminal in use by an API request, task or capability check · manual input paused")).toBeInTheDocument();
    else expect(screen.getByRole("alert")).toHaveTextContent("Close this tab and log in again");
  });
  it.each(["en", "zh-CN"] as const)("shows fixed disconnect reasons and never reconnects an ended session in %s", async (locale) => {
    const connected = { ...pending, workspaceId: "workspace-a", status: "connected" };
    api.list.mockResolvedValue([connected]);
    render(view("workspace-a", false, true, locale));
    fireEvent.click(await screen.findByText("Production node · 10.0.0.8:22"));
    expect(screen.getByTestId("temporary-pane")).not.toHaveAttribute("data-input-disabled", "true");
    const t = createTranslator(locale);
    for (const code of ["TEMPORARY_KEEPALIVE_TIMEOUT", "TEMPORARY_REMOTE_DISCONNECTED", "TEMPORARY_TRANSPORT_CLOSED", "TEMPORARY_REMOTE_SHELL_CLOSED"]) {
      api.list.mockResolvedValue([{ ...connected, status: "disconnected", errorCode: code }]);
      const message = t(`ssh.temporary.errors.${code}`);
      expect(message).not.toContain(code);
      await waitFor(() => expect(screen.getByRole("alert")).toHaveTextContent(message));
      expect(screen.getByTestId("temporary-pane")).toHaveAttribute("data-input-disabled", "true");
    }
    expect(api.claim).not.toHaveBeenCalled();
    expect(api.start).not.toHaveBeenCalled();
    expect(api.trust).not.toHaveBeenCalled();
  });
  it("does not display unknown peer text as a disconnect reason", async () => {
    const rawPeerText = "TEMPORARY_TRANSPORT_CLOSED private-gateway.invalid password=synthetic-secret";
    api.list.mockResolvedValue([{ ...pending, workspaceId: "workspace-a", status: "disconnected", errorCode: rawPeerText }]);
    render(view());
    fireEvent.click(await screen.findByText("Production node · 10.0.0.8:22"));
    expect(screen.getByRole("alert")).toHaveTextContent("Temporary session action failed");
    expect(screen.queryByText(/private-gateway|synthetic-secret/)).not.toBeInTheDocument();
    expect(api.start).not.toHaveBeenCalled();
  });
  it("requires explicit workspace claim and never starts on readiness", async () => {
    render(view());
    await screen.findByText("Connect in this workspace");
    expect(api.claim).not.toHaveBeenCalled(); expect(api.start).not.toHaveBeenCalled();
    fireEvent.click(screen.getByText("Connect in this workspace"));
    await waitFor(() => expect(api.start).toHaveBeenCalledWith("workspace-a", "opaque-fixture"));
    expect(api.claim).toHaveBeenCalledWith("workspace-a", "opaque-fixture");
  });
  it("claims a single-workspace launch once when SSH becomes visible", async () => {
    const mounted = render(view("workspace-a", true, false));
    await waitFor(() => expect(api.list).toHaveBeenCalled());
    expect(api.claim).not.toHaveBeenCalled();
    mounted.rerender(view("workspace-a", true));
    await waitFor(() => expect(api.start).toHaveBeenCalledWith("workspace-a", pending.handle));
    await waitFor(() => expect(api.list.mock.calls.length).toBeGreaterThan(1));
    expect(api.claim).toHaveBeenCalledTimes(1);
    expect(screen.queryByText("Connect in this workspace")).toBeNull();
    expect(api.trust).not.toHaveBeenCalled();
  });
  it("cancels a late claim after workspace unmount without connecting", async () => {
    let resolve!: (value: unknown) => void;
    api.claim.mockReturnValue(new Promise((done) => { resolve = done; }));
    const mounted = render(view());
    fireEvent.click(await screen.findByText("Connect in this workspace"));
    mounted.unmount();
    await act(async () => resolve({ ...pending, workspaceId: "workspace-a", status: "claimed" }));
    await waitFor(() => expect(api.cancel).toHaveBeenCalledWith("workspace-a", "opaque-fixture"));
    expect(api.start).not.toHaveBeenCalled();
    await waitFor(() => expect(api.close).toHaveBeenCalledWith("workspace-a"));
  });
  it("does not close live sessions during StrictMode readiness replay", async () => {
    render(<StrictMode>{view()}</StrictMode>);
    await screen.findByText("Connect in this workspace");
    await new Promise((resolve) => setTimeout(resolve, 10));
    expect(api.close).not.toHaveBeenCalled(); expect(api.start).not.toHaveBeenCalled();
  });
  it("identifies connected nodes without a second trust action and preserves the mounted saved view", async () => {
    api.list.mockResolvedValue([
      { ...pending, workspaceId: "workspace-a", status: "connected" },
      { ...pending, handle: "second-handle", workspaceId: "workspace-a", status: "connected", displayName: "Backup node", endpoint: "10.0.0.9:2222" },
    ]);
    render(view());
    fireEvent.click(await screen.findByText("Production node · 10.0.0.8:22"));
    expect(screen.getByText("Backup node · 10.0.0.9:2222")).toBeInTheDocument();
    expect(screen.getByTestId("temporary-pane")).toHaveTextContent("true");
    expect(screen.getByTestId("saved")).toBeInTheDocument();
    expect(api.trust).not.toHaveBeenCalled();
    expect(screen.queryByText("Trust this fingerprint once")).not.toBeInTheDocument();
    expect(screen.queryByText(/reconnect/i)).not.toBeInTheDocument();
  });
});
