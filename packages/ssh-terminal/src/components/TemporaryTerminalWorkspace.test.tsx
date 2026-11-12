// @vitest-environment jsdom
import { StrictMode } from "react";
import { act, cleanup, fireEvent, render, screen, waitFor } from "@testing-library/react";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import { I18nProvider } from "@unfour/ui";
import { TemporaryTerminalWorkspace } from "./TemporaryTerminalWorkspace";

const api = vi.hoisted(() => ({ list: vi.fn(), claim: vi.fn(), start: vi.fn(), cancel: vi.fn(), close: vi.fn(), output: vi.fn(), trust: vi.fn() }));
vi.mock("@unfour/command-client", () => ({
  listTemporarySsh: api.list, claimTemporarySsh: api.claim, startTemporarySsh: api.start,
  cancelTemporarySsh: api.cancel, closeTemporarySshWorkspace: api.close, readTemporarySshOutput: api.output, trustTemporarySsh: api.trust,
}));
vi.mock("./TerminalPane", () => ({ TerminalPane: ({ temporary }: { temporary: boolean }) => <div data-testid="temporary-pane">{String(temporary)}</div> }));
const pending = { handle: "opaque-fixture", workspaceId: null, status: "pending", expiresInSeconds: 120, fingerprint: null, errorCode: null };
function view(workspaceId = "workspace-a", autoClaim = false, surfaceActive = true) {
  return <I18nProvider initialLocale="en"><TemporaryTerminalWorkspace workspaceId={workspaceId} autoClaim={autoClaim} surfaceActive={surfaceActive}><div data-testid="saved">saved</div></TemporaryTerminalWorkspace></I18nProvider>;
}
describe("temporary terminal lifecycle", () => {
  afterEach(async () => { cleanup(); await new Promise((resolve) => setTimeout(resolve, 5)); });
  beforeEach(() => {
    vi.clearAllMocks();
    api.list.mockResolvedValue([pending]); api.output.mockResolvedValue([]);
    api.claim.mockResolvedValue({ ...pending, workspaceId: "workspace-a", status: "claimed" });
    api.start.mockResolvedValue(undefined); api.cancel.mockResolvedValue(undefined); api.close.mockResolvedValue(undefined); api.trust.mockResolvedValue(undefined);
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
  it("keeps fingerprint trust explicit and preserves the mounted saved view", async () => {
    api.list.mockResolvedValue([{ ...pending, workspaceId: "workspace-a", status: "awaiting-trust", fingerprint: "SHA256:synthetic" }]);
    render(view());
    fireEvent.click(await screen.findByText("Bastion temporary terminal"));
    expect(screen.getByTestId("temporary-pane")).toHaveTextContent("true");
    expect(screen.getByTestId("saved")).toBeInTheDocument();
    expect(api.trust).not.toHaveBeenCalled();
    fireEvent.click(screen.getByText("Trust this fingerprint once"));
    await waitFor(() => expect(api.trust).toHaveBeenCalledWith("workspace-a", "opaque-fixture", "SHA256:synthetic"));
    expect(screen.queryByText(/reconnect/i)).not.toBeInTheDocument();
  });
});
