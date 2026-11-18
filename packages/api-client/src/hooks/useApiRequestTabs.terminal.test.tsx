// @vitest-environment jsdom
import type { ReactNode } from "react";
import { act, cleanup, render, renderHook, screen, waitFor } from "@testing-library/react";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import { QueryClient, QueryClientProvider } from "@tanstack/react-query";
import { listTemporaryRoutes, sendApiRequest, type RequestExecutionResult } from "@unfour/command-client";
import { ApiTerminalExecutionNotice } from "../components/ApiTerminalExecutionNotice";
import { ResponseTabs } from "../components/ResponseTabs";
import { useApiRequestTabs } from "./useApiRequestTabs";
import { resetApiRequestTabStore } from "../model/api-request-tab-state";

vi.mock("@unfour/command-client", async (original) => ({
  ...await original<typeof import("@unfour/command-client")>(),
  listSavedApiRequests: vi.fn().mockResolvedValue([]), listApiHistory: vi.fn().mockResolvedValue([]),
  listWorkspaceEnvironments: vi.fn().mockResolvedValue([]), listSshConnections: vi.fn().mockResolvedValue([]),
  getApiRoutes: vi.fn().mockResolvedValue({ request: { mode: "inherit" }, environment: { mode: "direct" }, effective: { mode: "direct" }, environmentId: null }),
  listTemporaryRoutes: vi.fn(), sendApiRequest: vi.fn(),
}));
const session = { handle: "live-node", displayName: "Node", endpoint: "fixture.invalid:22" };
const execution: RequestExecutionResult = {
  response: { historyId: "h", status: 200, statusText: "OK", headers: [], body: '{"data":[1,2,3]}', durationMs: 1 },
  httpError: null, httpErrorCode: null,
  preRequest: { status: "skipped", console: [], tests: [], durationMs: 0, error: null },
  postResponse: { status: "skipped", console: [], tests: [], durationMs: 0, error: null },
};
beforeEach(() => { vi.mocked(listTemporaryRoutes).mockResolvedValue([session]); vi.mocked(sendApiRequest).mockResolvedValue(execution); });
afterEach(() => { cleanup(); resetApiRequestTabStore(); vi.clearAllMocks(); });
async function setup(temporaryExecutionMode: "tcp" | "terminalCurl" = "terminalCurl") {
  const client = new QueryClient({ defaultOptions: { queries: { retry: false }, mutations: { retry: false } } });
  const hook = renderHook(() => useApiRequestTabs("w"), {
    wrapper: ({ children }: { children: ReactNode }) => <QueryClientProvider client={client}>{children}</QueryClientProvider>,
  });
  act(() => hook.result.current.updateDraft("new:1", {
    url: "http://fixture.invalid/embeddings", method: "POST", bodyMode: "raw", rawBodyType: "json", body: '{"input":"北京"}',
    temporaryHandle: session.handle, temporaryExecutionMode,
  }));
  await waitFor(() => expect(hook.result.current.routing.ready).toBe(true));
  return { ...hook, client };
}
it("requires confirmation on every send, including retry, and transmits only runtime route arguments", async () => {
  const { result } = await setup();
  await act(async () => result.current.sendTab(result.current.activeTab!));
  expect(sendApiRequest).not.toHaveBeenCalled();
  expect(result.current.activeTab?.sendErrorCode).toBe("SSH_TERMINAL_HTTP_CONFIRMATION_REQUIRED");
  act(() => result.current.updateDraft("new:1", { terminalConfirmed: true }));
  await act(async () => { result.current.sendTab(result.current.activeTab!); result.current.sendTab(result.current.activeTab!); });
  expect(sendApiRequest).toHaveBeenCalledTimes(1);
  const sent = vi.mocked(sendApiRequest).mock.lastCall!;
  expect(sent[1]).toMatchObject({ method: "POST", body: '{"input":"北京"}', bodyKind: "json" });
  expect(sent[3]).toBe(session.handle);
  expect(sent[4]).toEqual({ temporaryExecutionMode: "terminalCurl", terminalConfirmed: true });
  expect(JSON.stringify([sent[1], sent[2]])).not.toMatch(/terminalCurl|terminalConfirmed|live-node/);
  expect(result.current.activeTab?.draft.terminalConfirmed).toBe(false);
  await waitFor(() => expect(result.current.activeTab?.sending).toBe(false));
  await act(async () => result.current.sendTab(result.current.activeTab!));
  expect(sendApiRequest).toHaveBeenCalledTimes(1);
});
it.each(["body", "temporaryHandle", "temporaryExecutionMode"] as const)("invalidates confirmation after editing %s", async (field) => {
  const { result } = await setup();
  act(() => result.current.updateDraft("new:1", { terminalConfirmed: true }));
  expect(result.current.activeTab?.draft.terminalConfirmed).toBe(true);
  const patches = { body: { body: '{"input":"changed"}' }, temporaryHandle: { temporaryHandle: "other-node" }, temporaryExecutionMode: { temporaryExecutionMode: "tcp" as const } };
  act(() => result.current.updateDraft("new:1", patches[field]));
  expect(result.current.activeTab?.draft.terminalConfirmed).toBe(false);
});
it.each(["terminalTaskBusy", "terminalRecoveryRequired"] as const)("rejects the execution boundary immediately when %s changes", async (flag) => {
  const { result, client } = await setup();
  act(() => result.current.updateDraft("new:1", { terminalConfirmed: true }));
  const send = result.current.sendTab, tab = result.current.activeTab!;
  await act(async () => { client.setQueryData(["api-live-sessions", "w"], [{ ...session, [flag]: true }]); send(tab); });
  expect(sendApiRequest).not.toHaveBeenCalled();
  expect(result.current.activeTab?.draft.terminalConfirmed).toBe(false);
});
it("discards confirmation when switching tabs", async () => {
  const { result } = await setup();
  act(() => result.current.updateDraft("new:1", { terminalConfirmed: true }));
  act(() => result.current.newRequest());
  act(() => result.current.selectTab("new:1"));
  expect(result.current.activeTab?.draft.terminalConfirmed).toBe(false);
});
it("blocks unsupported multipart before sending or invoking scripts", async () => {
  const { result } = await setup();
  act(() => result.current.updateDraft("new:1", { bodyMode: "multipart", multipartParts: [] }));
  act(() => result.current.updateDraft("new:1", { terminalConfirmed: true }));
  await act(async () => result.current.sendTab(result.current.activeTab!));
  expect(sendApiRequest).not.toHaveBeenCalled();
  expect(result.current.activeTab?.sendErrorCode).toBe("SSH_TERMINAL_HTTP_UNSUPPORTED_REQUEST");
});
it("retains a complete response when terminal restoration is unconfirmed", async () => {
  vi.mocked(sendApiRequest).mockResolvedValue({ ...execution, response: { ...execution.response!, warnings: ["SSH_TERMINAL_HTTP_TERMINAL_RECOVERY_REQUIRED"] } });
  const { result } = await setup();
  act(() => result.current.updateDraft("new:1", { terminalConfirmed: true }));
  await act(async () => result.current.sendTab(result.current.activeTab!));
  await waitFor(() => expect(result.current.activeTab?.response?.status).toBe(200));
  expect(result.current.activeTab?.response?.body).toBe(execution.response?.body);
  expect(result.current.activeTab?.sendError).toBeNull();
});
it("localizes fixed transfer errors and marks the remote result uncertain", async () => {
  vi.mocked(sendApiRequest).mockResolvedValue({ ...execution, response: null, httpErrorCode: "SSH_TERMINAL_HTTP_TIMEOUT", httpError: "untrusted-remote-message" });
  const { result } = await setup();
  act(() => result.current.updateDraft("new:1", { terminalConfirmed: true }));
  await act(async () => result.current.sendTab(result.current.activeTab!));
  await waitFor(() => expect(result.current.activeTab?.sending).toBe(false));
  expect(result.current.activeTab?.sendError).toContain("timed out");
  expect(result.current.activeTab?.sendError).not.toContain("untrusted-remote-message");
  expect(result.current.activeTab?.sendError).toContain("Remote effects may have occurred");
});

describe.each(["result", "rejection"])("send failure via %s", (path) => {
  it.each([
    ["SSH_TUNNEL_FORWARD_PROHIBITED", false, false],
    ["SSH_TUNNEL_INVALID_TARGET", false, false],
    ["NETWORK_ERROR", true, true],
    ["HTTP_ERROR", true, true],
    ["API_TIMEOUT", false, false],
    ["API_TIMEOUT", true, true],
    ["API_CANCELLED", false, false],
    ["API_CANCELLED", true, true],
    ["SSH_TUNNEL_TRANSPORT_CLOSED", true, true],
    ["SSH_TERMINAL_CANCELLED", undefined, false], // The dedicated terminal notice owns this warning.
  ] as const)("sets the warning for %s with forwarding stage %s", async (code, httpRequestMayHaveBeenSent, uncertainInError) => {
    if (path === "result") {
      vi.mocked(sendApiRequest).mockResolvedValue({ ...execution, response: null, httpErrorCode: code, httpError: "send failed", httpRequestMayHaveBeenSent });
    } else {
      vi.mocked(sendApiRequest).mockRejectedValue({ code, message: "send failed", httpRequestMayHaveBeenSent });
    }
    const terminal = code === "SSH_TERMINAL_CANCELLED";
    const { result } = await setup(terminal ? "terminalCurl" : "tcp");
    if (terminal) act(() => result.current.updateDraft("new:1", { terminalConfirmed: true }));
    await act(async () => result.current.sendTab(result.current.activeTab!));
    await waitFor(() => expect(result.current.activeTab?.sendErrorCode).toBe(code));
    expect(result.current.activeTab?.sendError?.includes("Remote effects may have occurred")).toBe(uncertainInError);
    if (code === "API_CANCELLED") {
      const tab = result.current.activeTab!;
      const view = (responseTab: "body" | "request") => <ResponseTabs tab={{ ...tab, responseTab }}
        onRetry={vi.fn()} onOpenAuthSettings={vi.fn()} onResponseTabChange={vi.fn()} />;
      const { rerender } = render(view("body"));
      expect(screen.queryAllByText(/Remote effects may have occurred/)).toHaveLength(uncertainInError ? 1 : 0);
      rerender(view("request"));
      expect(screen.queryAllByText(/Remote effects may have occurred/)).toHaveLength(uncertainInError ? 1 : 0);
    }
    expect(sendApiRequest).toHaveBeenCalledTimes(1);
  });
});

it.each([
  ["result", false], ["result", true], ["rejection", false], ["rejection", true],
] as const)("renders one cancellation warning via %s when recovery is %s", async (path, recoveryRequired) => {
  const code = "SSH_TERMINAL_CANCELLED";
  if (path === "result") {
    vi.mocked(sendApiRequest).mockResolvedValue({ ...execution, response: null, httpErrorCode: code, httpError: "cancelled" });
  } else {
    vi.mocked(sendApiRequest).mockRejectedValue({ code, message: "cancelled" });
  }
  const { result, client } = await setup();
  act(() => result.current.updateDraft("new:1", { terminalConfirmed: true }));
  await act(async () => result.current.sendTab(result.current.activeTab!));
  await waitFor(() => expect(result.current.activeTab?.sendErrorCode).toBe("SSH_TERMINAL_CANCELLED"));
  act(() => client.setQueryData(["api-live-sessions", "w"], [{ ...session, terminalRecoveryRequired: recoveryRequired }]));
  await waitFor(() => expect(result.current.routing.temporary.data?.[0]?.terminalRecoveryRequired).toBe(recoveryRequired));
  const tab = result.current.activeTab!;
  render(<>
    <ApiTerminalExecutionNotice active tab={tab} routing={result.current.routing} onUpdate={vi.fn()} />
    <ResponseTabs tab={tab} onRetry={vi.fn()} onOpenAuthSettings={vi.fn()} onResponseTabChange={vi.fn()} />
  </>);
  expect(screen.getByText("Request cancelled")).toBeInTheDocument();
  expect(screen.getAllByText(/Remote effects may have occurred/)).toHaveLength(1);
  expect(screen.queryAllByText(/Reconnect through the bastion/)).toHaveLength(recoveryRequired ? 1 : 0);
});

it.each(["result", "rejection"])("retains uncertain remote effects after an input acknowledgement timeout via %s without replay", async (path) => {
  const code = "SSH_TERMINAL_INPUT_ACK_TIMEOUT";
  if (path === "result") {
    vi.mocked(sendApiRequest).mockResolvedValue({ ...execution, response: null, httpErrorCode: code, httpError: "untrusted-remote-detail" });
  } else {
    vi.mocked(sendApiRequest).mockRejectedValue({ code, message: "untrusted-remote-detail" });
  }
  const { result, client } = await setup();
  act(() => result.current.updateDraft("new:1", { terminalConfirmed: true }));
  await act(async () => result.current.sendTab(result.current.activeTab!));
  await waitFor(() => expect(result.current.activeTab?.sending).toBe(false));
  expect(result.current.activeTab?.sendErrorCode).toBe(code);
  expect(result.current.activeTab?.sendError).toContain("Remote effects may have occurred");
  expect(result.current.activeTab?.sendError).not.toContain("untrusted-remote-detail");
  expect(result.current.activeTab?.response).toBeNull();
  expect(result.current.activeTab?.draft.terminalConfirmed).toBe(false);
  act(() => client.setQueryData(["api-live-sessions", "w"], [{ ...session, terminalRecoveryRequired: true }]));
  await waitFor(() => expect(result.current.routing.temporary.data?.[0]?.terminalRecoveryRequired).toBe(true));
  const tab = result.current.activeTab!;
  render(<>
    <ApiTerminalExecutionNotice active tab={tab} routing={result.current.routing} onUpdate={vi.fn()} />
    <ResponseTabs tab={tab} onRetry={vi.fn()} onOpenAuthSettings={vi.fn()} onResponseTabChange={vi.fn()} />
  </>);
  expect(screen.getAllByText(/Remote effects may have occurred/)).toHaveLength(1);
  expect(screen.getAllByText(/Reconnect through the bastion/)).toHaveLength(1);
  act(() => client.setQueryData(["api-live-sessions", "w"], [session]));
  await act(async () => result.current.sendTab(result.current.activeTab!));
  expect(sendApiRequest).toHaveBeenCalledTimes(1);
});

it.each(["failure", "response"])("retains the original terminal %s when recovery blocks retry", async (outcome) => {
  vi.mocked(sendApiRequest).mockResolvedValue(outcome === "failure"
    ? { ...execution, response: null, httpErrorCode: "SSH_TERMINAL_STARTUP_TIMEOUT", httpError: "untrusted detail" }
    : { ...execution, response: { ...execution.response!, warnings: ["SSH_TERMINAL_HTTP_TERMINAL_RECOVERY_REQUIRED"] } });
  const { result, client } = await setup();
  act(() => result.current.updateDraft("new:1", { terminalConfirmed: true }));
  await act(async () => result.current.sendTab(result.current.activeTab!));
  await waitFor(() => expect(result.current.activeTab?.sending).toBe(false));
  const original = result.current.activeTab!;
  act(() => client.setQueryData(["api-live-sessions", "w"], [{ ...session, terminalRecoveryRequired: true }]));
  await act(async () => result.current.sendTab(result.current.activeTab!));
  expect(sendApiRequest).toHaveBeenCalledTimes(1);
  expect(result.current.activeTab!.sendErrorCode).toBe(original.sendErrorCode);
  expect(result.current.activeTab!.response).toEqual(original.response);
  expect(result.current.activeTab!.execution).toEqual(original.execution);
});
