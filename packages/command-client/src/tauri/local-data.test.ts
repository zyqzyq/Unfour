// @vitest-environment jsdom
import { afterEach, expect, it, vi } from "vitest";
import { invoke } from "@tauri-apps/api/core";
import { isLocalDataQuery, readLocalDataVersions } from "./local-data";

vi.mock("@tauri-apps/api/core", () => ({ invoke: vi.fn() }));
afterEach(() => { delete window.__TAURI_INTERNALS__; vi.resetAllMocks(); });

it("uses the content-free native clock command only in Tauri", async () => {
  await expect(readLocalDataVersions()).resolves.toBeNull();
  expect(invoke).not.toHaveBeenCalled();
  window.__TAURI_INTERNALS__ = {};
  const clocks = [{ workspaceId: "ws", domain: "environments", version: "9007199254740993" }];
  vi.mocked(invoke).mockResolvedValue(clocks);
  await expect(readLocalDataVersions()).resolves.toEqual(clocks);
  expect(invoke).toHaveBeenCalledWith("local_data_versions", undefined);
});

it("matches only explicit query dependencies in the affected workspace", () => {
  const changes = [{ workspaceId: "ws", domain: "environments", version: "1" }];
  expect(isLocalDataQuery({ cloudSyncWorkspaceId: "ws", localDataDomains: ["environments"] }, changes)).toBe(true);
  expect(isLocalDataQuery({ cloudSyncWorkspaceId: "other", localDataDomains: ["environments"] }, changes)).toBe(false);
  expect(isLocalDataQuery({ cloudSyncWorkspaceId: "ws", localDataDomains: ["flows"] }, changes)).toBe(false);
  expect(isLocalDataQuery({ cloudSyncWorkspaceId: "ws" }, changes)).toBe(false);
  expect(isLocalDataQuery(undefined, changes)).toBe(false);
  expect(isLocalDataQuery({ localDataDomains: ["environments"] }, changes)).toBe(false);
  expect(isLocalDataQuery({ cloudSyncWorkspaceId: null, localDataDomains: ["workspaces"] }, changes)).toBe(false);
  expect(isLocalDataQuery({ cloudSyncWorkspaceId: null, localDataDomains: ["workspaces"] }, [{ ...changes[0], domain: "workspaces" }])).toBe(true);
});
