import { beforeEach, expect, it, vi } from "vitest";
import { call } from "./invoke";
import { cancelTemporarySshProbe, getTemporarySshProbe, startTemporarySshProbe, startTemporarySshTcpProbe } from "./temporary-ssh";

vi.mock("./invoke", () => ({ call: vi.fn(), isTauriRuntime: () => true }));
beforeEach(() => vi.clearAllMocks());

it.each(["transport", "curl", "terminal"] as const)("keeps %s as a distinct explicit probe kind without bypassing confirmation", async (kind) => {
  await startTemporarySshProbe("workspace", "handle", kind, false);
  expect(call).toHaveBeenCalledExactlyOnceWith("ssh_temporary_probe_start", {
    workspaceId: "workspace", handle: "handle", kind, confirmed: false,
  });
});

it("passes workspace, session, explicit confirmation and run identity to the probe commands", async () => {
  await startTemporarySshProbe("workspace", "handle", "sftp", true);
  await getTemporarySshProbe("workspace", "handle");
  await cancelTemporarySshProbe("workspace", "handle", "run-id");
  await startTemporarySshTcpProbe("workspace", "handle", "10.0.0.1", 18291, true);
  expect(vi.mocked(call).mock.calls).toEqual([
    ["ssh_temporary_probe_start", { workspaceId: "workspace", handle: "handle", kind: "sftp", confirmed: true }],
    ["ssh_temporary_probe_get", { workspaceId: "workspace", handle: "handle" }],
    ["ssh_temporary_probe_cancel", { workspaceId: "workspace", handle: "handle", runId: "run-id" }],
    ["ssh_temporary_probe_tcp_start", { workspaceId: "workspace", handle: "handle", host: "10.0.0.1", port: 18291, confirmed: true }],
  ]);
});
