// @vitest-environment jsdom
import { act, cleanup, fireEvent, render, screen, within } from "@testing-library/react";
import { afterEach, beforeEach, expect, it, vi } from "vitest";
import {
  browseRedisCollection, type RedisCollectionPage, type RedisCollectionRow, type RedisConnection,
} from "@unfour/command-client";
import { RedisCollectionBrowser } from "./RedisCollectionBrowser";

vi.mock("@unfour/command-client", async (load) => ({
  ...await load<typeof import("@unfour/command-client")>(), browseRedisCollection: vi.fn(),
}));
const browse = vi.mocked(browseRedisCollection);
const connection: RedisConnection = {
  workspaceId: "orders", id: "cache", revision: 2, host: "localhost", port: 6379,
  name: "Orders", dbIndex: 2, username: null, tls: false, hasPassword: false,
};
const key = btoa("order:123");
const target = { workspaceId: "orders", connectionId: "cache", keyBase64: key, expectedRevision: 2 };
const row = (memberBase64: string, options: Partial<RedisCollectionRow> = {}): RedisCollectionRow =>
  ({ memberBase64, index: null, score: null, valueBase64: null, ...options });
const read = () => fireEvent.click(screen.getByRole("button", { name: "Browse contents" }));

beforeEach(() => vi.resetAllMocks());
afterEach(cleanup);

it("allows continuing an empty HSCAN batch and selects the full binary field", async () => {
  const select = vi.fn();
  const binary = row("/w==", { valueBase64: "" });
  browse.mockResolvedValueOnce({ cursor: "t.7", rows: [] })
    .mockResolvedValueOnce({ cursor: "0", rows: [binary, binary] });
  render(<RedisCollectionBrowser connection={connection} keyBase64={key} kind="hash" onSelectHashField={select} />);
  fireEvent.change(screen.getByRole("textbox", { name: "Field pattern" }), { target: { value: "price*" } });
  read();
  expect(await screen.findByText("No members in this batch.")).toBeVisible();
  fireEvent.click(screen.getByRole("button", { name: "Next batch" }));
  const button = await screen.findByRole("button", { name: "HEX ff" });
  expect(browse).toHaveBeenLastCalledWith({ ...target, kind: "hash", cursor: "t.7", pattern: "price*", descending: false, startIndex: undefined });
  fireEvent.click(button);
  expect(select).toHaveBeenCalledWith("/w==");
  expect(screen.getByText("(empty)")).toBeVisible();
  expect(screen.queryByRole("button", { name: "Next batch" })).not.toBeInTheDocument();
});

it("accepts a starting rank, preserves the score text, and restarts after changing sort", async () => {
  const first = { cursor: "t.100", rows: [row(btoa("top"), { index: 50, score: "9007199254740992.5" })] };
  browse.mockResolvedValueOnce(first).mockResolvedValueOnce({ cursor: "0", rows: [] }).mockResolvedValueOnce(first);
  render(<RedisCollectionBrowser connection={connection} keyBase64={key} kind="zset" />);
  fireEvent.change(screen.getByRole("spinbutton", { name: "Start rank" }), { target: { value: "50" } });
  fireEvent.change(screen.getByRole("combobox", { name: "Score order" }), { target: { value: "desc" } });
  read();
  expect(await screen.findByText("9007199254740992.5")).toBeVisible();
  expect(browse).toHaveBeenCalledWith({ ...target, kind: "zset", cursor: "0", pattern: "*", descending: true, startIndex: 50 });
  expect(screen.getByRole("columnheader", { name: "Reverse rank" })).toBeVisible();
  fireEvent.click(screen.getByRole("button", { name: "Next batch" }));
  expect(await screen.findByText("No members in this batch.")).toBeVisible();
  expect(browse).toHaveBeenLastCalledWith({ ...target, kind: "zset", cursor: "t.100", pattern: "*", descending: true, startIndex: undefined });
  fireEvent.click(screen.getByRole("button", { name: "Previous batch" }));
  expect(await screen.findByText("9007199254740992.5")).toBeVisible();
  expect(browse).toHaveBeenLastCalledWith({ ...target, kind: "zset", cursor: "0", pattern: "*", descending: true, startIndex: 50 });
  fireEvent.change(screen.getByRole("combobox", { name: "Score order" }), { target: { value: "asc" } });
  expect(screen.queryByRole("table")).not.toBeInTheDocument();
});

it("does not collapse list elements at different indices", async () => {
  browse.mockResolvedValueOnce({ cursor: "0", rows: [row(btoa("same"), { index: 1 }), row(btoa("same"), { index: 2 })] });
  render(<RedisCollectionBrowser connection={connection} keyBase64={key} kind="list" />);
  read();
  await screen.findByRole("table");
  const rows = screen.getAllByRole("row");
  expect(within(rows[1]).getByText("1")).toBeVisible();
  expect(within(rows[2]).getByText("2")).toBeVisible();
  expect(screen.getAllByText("same")).toHaveLength(2);
});

it("clears obsolete rows if a continued read observes a replaced key", async () => {
  browse.mockResolvedValueOnce({ cursor: "t.7", rows: [row(btoa("previous member"))] })
    .mockRejectedValueOnce({ code: "REDIS_KEY_CHANGED" });
  render(<RedisCollectionBrowser connection={connection} keyBase64={key} kind="set" />);
  read();
  expect(await screen.findByText("previous member")).toBeVisible();
  fireEvent.click(screen.getByRole("button", { name: "Next batch" }));
  expect(await screen.findByRole("alert")).toHaveTextContent("key changed");
  expect(screen.queryByText("previous member")).not.toBeInTheDocument();
});

it("drops pending replies on target change and stops repeated scan continuations", async () => {
  let finish!: (result: RedisCollectionPage) => void;
  browse.mockImplementationOnce(() => new Promise((done) => { finish = done; }))
    .mockResolvedValueOnce({ cursor: "t.7", rows: [] }).mockResolvedValueOnce({ cursor: "t.7", rows: [] });
  const { rerender } = render(<RedisCollectionBrowser connection={connection} keyBase64={key} kind="set" />);
  read();
  rerender(<RedisCollectionBrowser connection={{ ...connection, revision: 3 }} keyBase64={key} kind="set" />);
  await act(async () => finish({ cursor: "0", rows: [row(btoa("obsolete"))] }));
  expect(screen.queryByText("obsolete")).not.toBeInTheDocument();
  read();
  fireEvent.click(await screen.findByRole("button", { name: "Next batch" }));
  expect(await screen.findByRole("alert")).toHaveTextContent("repeated a cursor");
  expect(screen.queryByRole("button", { name: "Next batch" })).not.toBeInTheDocument();
});
