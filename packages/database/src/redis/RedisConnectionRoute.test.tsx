// @vitest-environment jsdom
import { cleanup, fireEvent, render, screen, waitFor } from "@testing-library/react";
import { afterEach, beforeEach, expect, it, vi } from "vitest";
import { bindRedisSession, listSshConnections, listTemporaryRouteHandles, saveRedisConnection, testRedisConnectionInput, type RedisConnection, type SshConnection } from "@unfour/command-client";
import { RedisConnectionDialog } from "./RedisConnectionDialog";

vi.mock("@unfour/command-client", async (load) => ({
  ...await load<typeof import("@unfour/command-client")>(),
  listSshConnections: vi.fn(), saveRedisConnection: vi.fn(), testRedisConnectionInput: vi.fn(),
  listTemporaryRouteHandles: vi.fn().mockResolvedValue([]),
  bindRedisSession: vi.fn(),
}));
const cache: RedisConnection = {
  id: "cache", workspaceId: "w", name: "Cache", host: "cache.internal", port: 6379, username: null,
  dbIndex: 2, tls: true, allowWrites: true, hasPassword: true, revision: 5, sshConnectionId: null,
};
const jump: SshConnection = {
  id: "jump", workspaceId: "w", name: "Staging", host: "jump.internal", port: 22, username: "tester",
  authKind: "none", keyPath: null, credentialRef: null, createdAt: "", updatedAt: "",
  deletedAt: null, revision: 1, syncStatus: "", remoteId: null,
};
beforeEach(() => vi.resetAllMocks());
afterEach(cleanup);

it("keeps the temporary target password separate from the saved Redis password", async () => {
  vi.mocked(listTemporaryRouteHandles).mockResolvedValue(["session-12345678"]);
  vi.mocked(listSshConnections).mockResolvedValue([]);
  const stale = { ...cache, sshConnectionId: "deleted-jump", routeNeedsReview: true };
  render(<RedisConnectionDialog workspaceId="w" original={stale} latest={stale} initialName="" onClose={vi.fn()} onSaved={vi.fn()} />);
  await screen.findByRole("option", { name: "Bastion session · 12345678" });
  fireEvent.change(screen.getByLabelText("Network route"), { target: { value: "" } });
  fireEvent.change(screen.getByLabelText("Network route"), { target: { value: "live:session-12345678" } });
  expect(screen.getByRole("button", { name: "Test connection" })).toBeDisabled();
  fireEvent.change(screen.getByLabelText("Redis password for this session"), { target: { value: "runtime-only" } });
  fireEvent.click(screen.getByRole("button", { name: "Test connection" }));
  await waitFor(() => expect(testRedisConnectionInput).toHaveBeenCalledWith(expect.objectContaining({ sshConnectionId: "deleted-jump" }), "runtime-only", "session-12345678"));
  fireEvent.click(screen.getByRole("button", { name: "Save", exact: true }));
  await waitFor(() => expect(bindRedisSession).toHaveBeenCalledWith("w", cache.id, cache.revision, "session-12345678", "runtime-only"));
  expect(saveRedisConnection).not.toHaveBeenCalled();
});

it("tests and saves the selected SSH route, requiring a new password after retargeting", async () => {
  vi.mocked(listSshConnections).mockResolvedValue([jump, { ...jump, workspaceId: "other", id: "foreign", name: "Other" }]);
  render(<RedisConnectionDialog workspaceId="w" original={cache} latest={cache} initialName="" onClose={vi.fn()} onSaved={vi.fn()} />);
  await screen.findByRole("option", { name: /Staging/ });
  expect(screen.queryByRole("option", { name: /Other/ })).not.toBeInTheDocument();
  fireEvent.change(screen.getByLabelText("Network route"), { target: { value: "jump" } });
  expect(screen.getByLabelText("Allow manual Redis writes on this device")).not.toBeChecked();
  expect(screen.getByRole("button", { name: "Save", exact: true })).toBeDisabled();
  fireEvent.change(screen.getByLabelText("Password", { exact: true }), { target: { value: "new-password" } });
  fireEvent.click(screen.getByRole("button", { name: "Test connection" }));
  await waitFor(() => expect(testRedisConnectionInput).toHaveBeenCalledWith(expect.objectContaining({
    sshConnectionId: "jump", expectedSshRevision: 1, host: "cache.internal", tls: true, dbIndex: 2, allowWrites: false,
  }), "new-password"));
  fireEvent.click(screen.getByRole("button", { name: "Save", exact: true }));
  await waitFor(() => expect(saveRedisConnection).toHaveBeenCalledWith(expect.objectContaining({ sshConnectionId: "jump", expectedSshRevision: 1 }), "new-password"));
});

it("preserves an unavailable reference until the user explicitly chooses another route", async () => {
  vi.mocked(listSshConnections).mockResolvedValue([]);
  const missing = { ...cache, sshConnectionId: "removed" };
  render(<RedisConnectionDialog workspaceId="w" original={missing} latest={missing} initialName="" onClose={vi.fn()} onSaved={vi.fn()} />);
  expect(await screen.findByRole("option", { name: "SSH connection unavailable" })).toBeInTheDocument();
  expect(screen.getByLabelText("Network route")).toHaveValue("removed");
  expect(screen.getByRole("button", { name: "Save", exact: true })).toBeDisabled();
});
