import { beforeEach, expect, it, vi } from "vitest";
import { call } from "./invoke";
import { sendApiRequest } from "./api";

vi.mock("./invoke", () => ({ call: vi.fn(), isTauriRuntime: () => true }));
beforeEach(() => vi.clearAllMocks());
const input = { workspaceId: "w", method: "POST", url: "http://fixture.invalid", headers: [], query: [], body: "{}", bodyKind: "json" };
it("keeps legacy direct and temporary TCP argument shapes", async () => {
  await sendApiRequest("one", input);
  await sendApiRequest("two", input, undefined, "live");
  expect(vi.mocked(call).mock.calls).toEqual([
    ["api_send_request_v2", { executionId: "one", input, routing: undefined }],
    ["api_send_request_v2", { executionId: "two", input, routing: undefined, temporaryHandle: "live" }],
  ]);
});
it("sends explicit mode and confirmation separately from saved input", async () => {
  await sendApiRequest("id", input, undefined, "live", { temporaryExecutionMode: "terminalCurl", terminalConfirmed: true });
  expect(call).toHaveBeenCalledWith("api_send_request_v2", { executionId: "id", input, routing: undefined, temporaryHandle: "live", temporaryExecutionMode: "terminalCurl", terminalConfirmed: true });
  expect(input).not.toHaveProperty("temporaryExecutionMode");
});
