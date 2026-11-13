// @vitest-environment jsdom
import { cleanup, fireEvent, render, screen, waitFor, within } from "@testing-library/react";
import { afterEach, beforeEach, expect, it, vi } from "vitest";
import { mutateRedisKey, type RedisConnection, type RedisKeyInfo } from "@unfour/command-client";
import { RedisWriteDialog } from "./RedisWriteDialog";
import { RedisConnectionDialog } from "./RedisConnectionDialog";
import { RedisKeyBrowser } from "./RedisKeyBrowser";
import { initialWriteForm, prepareWrite, type WriteTarget } from "./redis-write";
import { redisName, textKey } from "./redis-bytes";

vi.mock("@unfour/command-client", async (load) => ({
  ...await load<typeof import("@unfour/command-client")>(), mutateRedisKey: vi.fn(),
}));
const connection: RedisConnection = {
  workspaceId: "w", id: "r", name: "Cache", host: "localhost", port: 6379,
  username: null, dbIndex: 2, tls: false, hasPassword: false, revision: 4, allowWrites: true,
};
const info: RedisKeyInfo = {
  exists: true, keyType: "string", pttlMs: 5000, length: 2, fieldFound: null,
  preview: { base64: btoa("89"), truncated: false },
};
const encoded = btoa("price:1");

function open(target: WriteTarget) {
  const resolved = vi.fn();
  render(<RedisWriteDialog connection={connection} target={target} onResolve={resolved} onClose={vi.fn()} onRefreshConnections={vi.fn()} />);
  return { resolved, dialog: within(screen.getByRole("dialog")) };
}
beforeEach(() => vi.resetAllMocks());
afterEach(cleanup);

it("requires an explicit local grant and revokes it when the target changes", () => {
  const { rerender } = render(<RedisKeyBrowser connection={{ ...connection, allowWrites: undefined }} />);
  expect(screen.queryByRole("button", { name: "New key" })).not.toBeInTheDocument();
  rerender(<RedisConnectionDialog workspaceId="w" original={connection} latest={connection} initialName="" onClose={vi.fn()} onSaved={vi.fn()} />);
  const allow = screen.getByLabelText("Allow manual Redis writes on this device");
  expect(allow).toBeChecked();
  fireEvent.change(screen.getByLabelText("Host"), { target: { value: "elsewhere.test" } });
  expect(allow).not.toBeChecked();
});

it("creates a reviewed String with a positive TTL and never dispatches invalid expiry", async () => {
  const { dialog, resolved } = open({ kind: "createString", keyBase64: encoded });
  fireEvent.change(dialog.getByLabelText("New value"), { target: { value: "" } });
  const ttl = dialog.getByLabelText("Time to live (seconds, optional)");
  fireEvent.change(ttl, { target: { value: "0" } });
  fireEvent.click(dialog.getByRole("button", { name: "Review change" }));
  expect(dialog.getByRole("alert")).toHaveTextContent("positive duration");
  fireEvent.change(ttl, { target: { value: "1.001" } });
  fireEvent.click(dialog.getByRole("button", { name: "Review change" }));
  expect(dialog.getByText("SET · NX · PX")).toBeVisible();
  expect(mutateRedisKey).not.toHaveBeenCalled();
  fireEvent.click(dialog.getByRole("button", { name: "Apply change" }));
  await waitFor(() => expect(resolved).toHaveBeenCalledTimes(1));
  expect(mutateRedisKey).toHaveBeenCalledWith({ workspaceId: "w", connectionId: "r", keyBase64: encoded, expectedRevision: 4,
    confirmed: true, mutation: { kind: "createString", valueBase64: "", ttlMs: 1001 } });
});

it("retains exact bytes for a binary value, and blocks replay after an unknown result", async () => {
  vi.mocked(mutateRedisKey).mockRejectedValue({ code: "REDIS_WRITE_OUTCOME_UNKNOWN" });
  const { dialog, resolved } = open({ kind: "updateString", keyBase64: encoded, info: { ...info, preview: { base64: "/wA=", truncated: false } } });
  const value = dialog.getByLabelText("New value (HEX bytes)");
  expect(value).toHaveValue("ff 00");
  fireEvent.change(value, { target: { value: "00 ff" } });
  fireEvent.click(dialog.getByRole("button", { name: "Review change" }));
  fireEvent.click(dialog.getByRole("button", { name: "Apply change" }));
  await dialog.findByRole("alert");
  expect(mutateRedisKey).toHaveBeenCalledWith({ workspaceId: "w", connectionId: "r", keyBase64: encoded, expectedRevision: 4,
    confirmed: true, mutation: { kind: "updateString", expectedBase64: "/wA=", valueBase64: "AP8=" } });
  expect(dialog.queryByRole("button", { name: "Apply change" })).not.toBeInTheDocument();
  fireEvent.click(dialog.getByRole("button", { name: "Read current state" }));
  expect(resolved).toHaveBeenCalledWith(expect.anything(), "reread");
  expect(mutateRedisKey).toHaveBeenCalledTimes(1);
});

it("warns that deleting the final field removes the key and preserves binary field identity", async () => {
  const { dialog } = open({ kind: "deleteHashField", keyBase64: encoded, fieldBase64: "/w==", info: { ...info, keyType: "hash", fieldFound: true } });
  expect(dialog.getByText(/last remaining field/)).toBeVisible();
  expect(dialog.getByRole("button", { name: "Cancel" })).toHaveFocus();
  fireEvent.click(dialog.getByRole("button", { name: "Apply change" }));
  await waitFor(() => expect(mutateRedisKey).toHaveBeenCalledWith({ workspaceId: "w", connectionId: "r", keyBase64: encoded, expectedRevision: 4,
    confirmed: true, mutation: { kind: "deleteHashField", fieldBase64: "/w==", expectedBase64: btoa("89") } }));
});

it("distinguishes binary and literal HEX names before a destructive field change", () => {
  const { dialog } = open({
    kind: "deleteHashField", keyBase64: textKey("HEX ff"), fieldBase64: "/w==",
    info: { ...info, keyType: "hash", fieldFound: true },
  });
  expect(dialog.getByText('"HEX ff"')).toBeVisible();
  expect(dialog.getByText("Hash field · HEX ff")).toBeVisible();
  const names = ["", "(empty key)", "a b", "a  b", " a", "a ", "a\nb", "a\\nb", "\u200ba", "a"];
  const labels = names.map((name) => redisName(textKey(name)));
  expect(new Set(labels).size).toBe(names.length);
  expect(redisName(textKey("a  b"))).toBe('"a\\u0020\\u0020b"');
});

it("removes a hidden write dialog on tab or workspace change", () => {
  const { rerender } = render(<RedisKeyBrowser connection={connection} />);
  fireEvent.click(screen.getByRole("button", { name: "New key" }));
  expect(screen.getByRole("dialog")).toBeVisible();
  rerender(<RedisKeyBrowser connection={connection} active={false} />);
  expect(screen.queryByRole("dialog")).not.toBeInTheDocument();
  rerender(<RedisKeyBrowser connection={{ ...connection, workspaceId: "other" }} />);
  expect(screen.queryByRole("dialog")).not.toBeInTheDocument();
  expect(mutateRedisKey).not.toHaveBeenCalled();
});

it("builds complete Hash and lifetime operations and rejects truncated source values", () => {
  const hash = { ...info, keyType: "hash" };
  const make = (target: WriteTarget, value = "2") => prepareWrite(target, { ...initialWriteForm(target), value, field: "price", seconds: "4" });
  expect(make({ kind: "createHash", keyBase64: encoded }).ready?.mutation).toEqual({ kind: "createHash", fieldBase64: btoa("price"), valueBase64: btoa("2") });
  expect(make({ kind: "addHashField", keyBase64: encoded, info: hash }).ready?.mutation.kind).toBe("addHashField");
  expect(make({ kind: "updateHashField", keyBase64: encoded, info: hash, fieldBase64: "" }).ready?.mutation).toEqual({
    kind: "updateHashField", fieldBase64: "", expectedBase64: btoa("89"), valueBase64: btoa("2"),
  });
  for (const kind of ["persist", "deleteKey", "setTtl"] as const) {
    expect(make({ kind, keyBase64: encoded, info: hash }).ready?.mutation).toEqual({ kind, expectedType: "hash", ...(kind === "setTtl" && { ttlMs: 4000 }) });
  }
  expect(make({ kind: "updateString", keyBase64: encoded, info: { ...info, preview: { base64: btoa("incomplete"), truncated: true } } }).ready).toBeUndefined();
});
