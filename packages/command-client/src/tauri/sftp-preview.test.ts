// @vitest-environment jsdom
import { afterEach, expect, it, vi } from "vitest";
import { previewSftpText } from "./ssh";

const invoke = vi.hoisted(() => vi.fn());
vi.mock("@tauri-apps/api/core", () => ({ invoke, Channel: vi.fn() }));
afterEach(() => { vi.unstubAllGlobals(); invoke.mockReset(); });

it("returns ephemeral text through the scoped read command without logging or transfer calls", async () => {
  vi.stubGlobal("__TAURI_INTERNALS__", {});
  const input = { workspaceId: "workspace", sessionId: "session", path: "/.env" };
  const result = { content: "PRIVATE_FIXTURE_CONTENT", size: 23 };
  invoke.mockResolvedValue(result);
  expect(await previewSftpText(input)).toEqual(result);
  expect(invoke).toHaveBeenCalledExactlyOnceWith("ssh_sftp_preview_text", { input });
});
