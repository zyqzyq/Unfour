import { beforeEach, describe, expect, it, vi } from "vitest";

const mocks = vi.hoisted(() => ({
  invoke: vi.fn(), getCurrent: vi.fn(), onOpenUrl: vi.fn(), stop: vi.fn(),
  listener: undefined as ((urls: string[]) => void) | undefined,
}));
vi.mock("@tauri-apps/api/core", () => ({ invoke: mocks.invoke }));
vi.mock("@tauri-apps/plugin-deep-link", () => ({ getCurrent: mocks.getCurrent, onOpenUrl: mocks.onOpenUrl }));

const callback = "unfour://auth/callback?authorizationCode=" + "C".repeat(43) + "&state=" + "S".repeat(43);
const snapshot = { account: { kind: "signedOut" }, syncContext: { kind: "inactive" } };

beforeEach(() => {
  vi.resetModules();
  vi.clearAllMocks();
  mocks.getCurrent.mockResolvedValue(null);
  mocks.invoke.mockResolvedValue(snapshot);
  mocks.onOpenUrl.mockImplementation(async (listener: (urls: string[]) => void) => {
    mocks.listener = listener;
    return mocks.stop;
  });
});

describe("account callback delivery", () => {
  it("deduplicates concurrent cold-start and live delivery", async () => {
    let resolve!: (value: typeof snapshot) => void;
    mocks.invoke.mockReturnValue(new Promise((done) => { resolve = done; }));
    mocks.getCurrent.mockResolvedValue([callback]);
    const { listenForAccountDeepLinks } = await import("./accountApi");
    const onState = vi.fn();
    const listening = listenForAccountDeepLinks(onState, vi.fn());
    await vi.waitFor(() => expect(mocks.invoke).toHaveBeenCalledTimes(1));
    mocks.listener?.([callback]);
    resolve(snapshot);
    await listening;
    await vi.waitFor(() => expect(onState).toHaveBeenCalled());
    expect(mocks.invoke).toHaveBeenCalledTimes(1);
  });

  it("retains the listener after failure and never retries the same callback", async () => {
    const failure = { code: "api_timeout", stage: "codeExchange" };
    mocks.invoke.mockRejectedValueOnce(failure);
    mocks.getCurrent.mockResolvedValue([callback]);
    const { listenForAccountDeepLinks } = await import("./accountApi");
    const onError = vi.fn();
    await listenForAccountDeepLinks(vi.fn(), onError);
    expect(onError).toHaveBeenCalledWith(failure);
    expect(mocks.stop).not.toHaveBeenCalled();
    mocks.listener?.([callback, callback.replace("S".repeat(43), "N".repeat(43))]);
    await vi.waitFor(() => expect(mocks.invoke).toHaveBeenCalledTimes(2));
    expect(onError).toHaveBeenCalledTimes(1);
  });

  it("keeps processing a batch after one callback fails", async () => {
    mocks.invoke.mockRejectedValueOnce({ code: "state_mismatch" }).mockResolvedValue(snapshot);
    mocks.getCurrent.mockResolvedValue([callback, callback.replace("S".repeat(43), "N".repeat(43))]);
    const { listenForAccountDeepLinks } = await import("./accountApi");
    const onState = vi.fn();
    await listenForAccountDeepLinks(onState, vi.fn());
    expect(mocks.invoke).toHaveBeenCalledTimes(2);
    expect(onState).toHaveBeenCalledWith(snapshot);
  });

  it("keeps the runtime listener when cold-start lookup fails", async () => {
    mocks.getCurrent.mockRejectedValue(new Error("private platform input"));
    const { listenForAccountDeepLinks } = await import("./accountApi");
    const onError = vi.fn();
    const stop = await listenForAccountDeepLinks(vi.fn(), onError);
    expect(onError).toHaveBeenCalledWith({ code: "callback_listener_failed", stage: "callbackListener" });
    expect(mocks.stop).not.toHaveBeenCalled();
    mocks.listener?.([callback]);
    await vi.waitFor(() => expect(mocks.invoke).toHaveBeenCalledTimes(1));
    stop();
    expect(mocks.stop).toHaveBeenCalledTimes(1);
  });

  it("sanitizes registration failures and ignores unrelated links", async () => {
    const { listenForAccountDeepLinks } = await import("./accountApi");
    mocks.onOpenUrl.mockRejectedValueOnce(new Error(callback));
    await expect(listenForAccountDeepLinks(vi.fn(), vi.fn())).rejects.toEqual({
      code: "callback_listener_failed", stage: "callbackListener",
    });
    mocks.getCurrent.mockResolvedValue(["unfour://workspace/open", "malformed", "https://example.test/auth/callback"]);
    await listenForAccountDeepLinks(vi.fn(), vi.fn());
    expect(mocks.invoke).not.toHaveBeenCalled();
  });
});
