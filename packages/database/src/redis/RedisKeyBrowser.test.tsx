// @vitest-environment jsdom
import { act, cleanup, fireEvent, render, screen } from "@testing-library/react";
import { afterEach, beforeEach, expect, it, vi } from "vitest";
import { inspectRedisKey, scanRedisKeys, type RedisConnection, type RedisKeyInfo } from "@unfour/command-client";
import { RedisKeyBrowser } from "./RedisKeyBrowser";

vi.mock("@unfour/command-client", async (load) => ({
  ...await load<typeof import("@unfour/command-client")>(), inspectRedisKey: vi.fn(), scanRedisKeys: vi.fn(),
}));
const connection: RedisConnection = { workspaceId: "w", id: "r", name: "Cache", host: "localhost", port: 6379, username: null, dbIndex: 1, tls: false, revision: 1, hasPassword: false };
const info: RedisKeyInfo = { exists: true, keyType: "string", length: 5, pttlMs: -1, fieldFound: null, preview: { base64: btoa("value"), truncated: false } };
beforeEach(() => vi.resetAllMocks());
afterEach(cleanup);

it("looks up the exact key without scan permission, and discards an obsolete read", async () => {
  let resolve!: (result: RedisKeyInfo) => void;
  vi.mocked(inspectRedisKey).mockImplementationOnce(() => new Promise((done) => { resolve = done; }));
  render(<RedisKeyBrowser connection={connection} />);
  fireEvent.change(screen.getByRole("textbox", { name: "Exact key name" }), { target: { value: "price:1" } });
  fireEvent.click(screen.getByRole("button", { name: "Inspect key" }));
  expect(inspectRedisKey).toHaveBeenCalledWith("w", "r", btoa("price:1"), undefined, 1);
  fireEvent.change(screen.getByRole("textbox", { name: "Exact key name" }), { target: { value: "price:2" } });
  await act(async () => resolve(info));
  expect(screen.queryByText("value")).not.toBeInTheDocument();
  vi.mocked(inspectRedisKey).mockResolvedValueOnce({ ...info, exists: false });
  fireEvent.click(screen.getByRole("button", { name: "Inspect key" }));
  expect(await screen.findByText(/price:2.*missing or expired/)).toBeVisible();
  expect(scanRedisKeys).not.toHaveBeenCalled();
});

it("continues an empty non-final batch, deduplicates binary keys and reads the original bytes", async () => {
  vi.mocked(scanRedisKeys).mockResolvedValueOnce({ cursor: "4", keys: [] }).mockResolvedValueOnce({ cursor: "0", keys: ["/w==", "/w=="] });
  vi.mocked(inspectRedisKey).mockResolvedValue(info);
  render(<RedisKeyBrowser connection={connection} onCreateFlow={vi.fn()} />);
  fireEvent.click(screen.getByRole("button", { name: "Browse by pattern" }));
  fireEvent.click(screen.getByRole("button", { name: "Scan", exact: true }));
  fireEvent.click(await screen.findByRole("button", { name: "Continue scan" }));
  expect(scanRedisKeys).toHaveBeenLastCalledWith("w", "r", "4", "*", 1);
  fireEvent.click(await screen.findByRole("button", { name: "HEX ff" }));
  expect(inspectRedisKey).toHaveBeenCalledWith("w", "r", "/w==", undefined, 1);
  expect(await screen.findByText("No expiry")).toBeVisible();
  expect(screen.getByText("value")).toBeVisible();
  expect(screen.queryByRole("button", { name: "Create Flow from this key" })).not.toBeInTheDocument();
  expect(screen.queryByRole("button", { name: "Continue scan" })).not.toBeInTheDocument();
});

it("seeds a Flow draft with a UTF-8 key and the scoped target only", async () => {
  vi.mocked(inspectRedisKey).mockResolvedValue(info);
  const create = vi.fn();
  render(<RedisKeyBrowser connection={connection} onCreateFlow={create} />);
  fireEvent.change(screen.getByRole("textbox", { name: "Exact key name" }), { target: { value: "商品:42" } });
  fireEvent.click(screen.getByRole("button", { name: "Inspect key" }));
  fireEvent.click(await screen.findByRole("button", { name: "Create Flow from this key" }));
  expect(create).toHaveBeenCalledWith({ workspaceId: "w", connectionId: "r", key: "商品:42" });
});

it.each(['{"id":9007199254740993}', '{"x":1,"x":2}'])("preserves exact payload data when JSON formatting would alter %s", async (text) => {
  vi.mocked(inspectRedisKey).mockResolvedValue({ ...info, preview: { base64: btoa(text), truncated: false } });
  render(<RedisKeyBrowser connection={connection} />);
  fireEvent.click(screen.getByRole("button", { name: "Inspect key" }));
  expect(await screen.findByText(text)).toBeVisible();
  expect(screen.queryByRole("option", { name: "JSON" })).not.toBeInTheDocument();
});

it("stops retaining key bytes when repeated large scan batches reach the display budget", async () => {
  const keys = Array.from({ length: 12 }, (_, index) => btoa("k".repeat(50_000) + index));
  for (const start of [0, 4, 8]) {
    vi.mocked(scanRedisKeys).mockResolvedValueOnce({ cursor: String(start + 4), keys: keys.slice(start, start + 4) });
  }
  render(<RedisKeyBrowser connection={connection} />);
  fireEvent.click(screen.getByRole("button", { name: "Browse by pattern" }));
  fireEvent.click(screen.getByRole("button", { name: "Scan", exact: true }));
  fireEvent.click(await screen.findByRole("button", { name: "Continue scan" }));
  await screen.findByText(/8 unique keys/);
  fireEvent.click(screen.getByRole("button", { name: "Continue scan" }));
  expect(await screen.findByText(/display limit was reached/)).toBeVisible();
  expect(screen.getAllByRole("listitem").length).toBeLessThan(keys.length);
  expect(screen.queryByRole("button", { name: "Continue scan" })).not.toBeInTheDocument();
});

it("can retry a hash field read without scanning or losing the key after a denied read", async () => {
  const hash = { ...info, keyType: "hash", preview: null };
  vi.mocked(inspectRedisKey).mockResolvedValueOnce(hash)
    .mockRejectedValueOnce({ code: "REDIS_PERMISSION_DENIED" })
    .mockResolvedValueOnce({ ...hash, fieldFound: false });
  render(<RedisKeyBrowser connection={connection} />);
  fireEvent.change(screen.getByRole("textbox", { name: "Exact key name" }), { target: { value: "hash" } });
  fireEvent.click(screen.getByRole("button", { name: "Inspect key" }));
  fireEvent.change(await screen.findByRole("textbox", { name: "Hash field" }), { target: { value: "price" } });
  fireEvent.click(screen.getByRole("button", { name: "Read field" }));
  expect(await screen.findByRole("alert")).toHaveTextContent("permissions");
  fireEvent.click(screen.getByRole("button", { name: "Read field" }));
  expect(await screen.findByText("Field does not exist.")).toBeVisible();
  expect(inspectRedisKey).toHaveBeenLastCalledWith("w", "r", btoa("hash"), btoa("price"), 1);
  expect(scanRedisKeys).not.toHaveBeenCalled();
});

it.each(["workspace", "revision"] as const)("discards pending key and scan replies when the %s changes", async (change) => {
  const next = change === "workspace" ? { ...connection, workspaceId: "other" } : { ...connection, host: "new-host", revision: 2 };
  let finishRead!: (value: RedisKeyInfo) => void;
  let finishScan!: (value: { cursor: string; keys: string[] }) => void;
  vi.mocked(inspectRedisKey).mockImplementationOnce(() => new Promise((done) => { finishRead = done; }));
  vi.mocked(scanRedisKeys).mockImplementationOnce(() => new Promise((done) => { finishScan = done; }));
  const { rerender } = render(<RedisKeyBrowser connection={connection} />);
  fireEvent.click(screen.getByRole("button", { name: "Inspect key" }));
  fireEvent.click(screen.getByRole("button", { name: "Browse by pattern" }));
  fireEvent.click(screen.getByRole("button", { name: "Scan", exact: true }));
  rerender(<RedisKeyBrowser connection={next} />);
  await act(async () => {
    finishRead(info);
    finishScan({ cursor: "42", keys: [btoa("old-host-key")] });
  });
  expect(screen.queryByText("value")).not.toBeInTheDocument();
  fireEvent.click(screen.getByRole("button", { name: "Browse by pattern" }));
  expect(screen.queryByText("old-host-key")).not.toBeInTheDocument();
  expect(screen.queryByRole("button", { name: "Continue scan" })).not.toBeInTheDocument();
  vi.mocked(scanRedisKeys).mockResolvedValueOnce({ cursor: "0", keys: [] });
  fireEvent.click(screen.getByRole("button", { name: "Scan", exact: true }));
  await screen.findByText(/Scan complete/);
  expect(scanRedisKeys).toHaveBeenLastCalledWith(next.workspaceId, next.id, "0", "*", next.revision);
});
