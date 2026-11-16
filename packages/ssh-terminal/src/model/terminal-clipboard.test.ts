// @vitest-environment jsdom
import { afterEach, describe, expect, it, vi } from "vitest";
import { readTerminalClipboard } from "./terminal-clipboard";

afterEach(() => vi.restoreAllMocks());

function permissions(state: PermissionState) {
  const query = vi.fn().mockResolvedValue({ state });
  Object.defineProperty(navigator, "permissions", { configurable: true, value: { query } });
  return query;
}

describe("terminal clipboard permission recovery", () => {
  it("recovers the first permission-prompt focus failure and reads exactly once more", async () => {
    const readText = vi.fn()
      .mockRejectedValueOnce(new DOMException("Document is not focused", "NotAllowedError"))
      .mockResolvedValueOnce("clipboard command");
    const query = permissions("granted");
    vi.spyOn(document, "hasFocus").mockReturnValue(true);
    const focus = vi.fn();

    await expect(readTerminalClipboard({ readText }, () => true, focus)).resolves.toBe("clipboard command");
    expect(readText).toHaveBeenCalledTimes(2);
    expect(query).toHaveBeenCalledExactlyOnceWith({ name: "clipboard-read" });
    expect(focus).toHaveBeenCalledOnce();
  });

  it.each(["denied", "prompt"] as const)("does not retry when permission remains %s", async (state) => {
    const failure = new DOMException("Not allowed", "NotAllowedError");
    const readText = vi.fn().mockRejectedValue(failure);
    permissions(state);
    const focus = vi.fn();

    await expect(readTerminalClipboard({ readText }, () => true, focus)).rejects.toBe(failure);
    expect(readText).toHaveBeenCalledOnce();
    expect(focus).not.toHaveBeenCalled();
  });

  it("preserves a second failure without repeatedly requesting clipboard access", async () => {
    const failure = new DOMException("Unavailable", "NotAllowedError");
    const readText = vi.fn().mockRejectedValue(failure);
    permissions("granted");
    vi.spyOn(document, "hasFocus").mockReturnValue(true);

    await expect(readTerminalClipboard({ readText }, () => true, vi.fn())).rejects.toBe(failure);
    expect(readText).toHaveBeenCalledTimes(2);
  });

  it("never reads again if the target changed while permission was being checked", async () => {
    const failure = new DOMException("Not allowed", "NotAllowedError");
    const readText = vi.fn().mockRejectedValue(failure);
    let current = true;
    permissions("granted").mockImplementation(async () => {
      current = false;
      return { state: "granted" };
    });
    const focus = vi.fn();

    await expect(readTerminalClipboard({ readText }, () => current, focus)).rejects.toBe(failure);
    expect(readText).toHaveBeenCalledOnce();
    expect(focus).not.toHaveBeenCalled();
  });

  it("does not retry while the app is still unfocused", async () => {
    const failure = new DOMException("Document is not focused", "NotAllowedError");
    const readText = vi.fn().mockRejectedValue(failure);
    permissions("granted");
    vi.spyOn(document, "hasFocus").mockReturnValue(false);

    await expect(readTerminalClipboard({ readText }, () => true, vi.fn())).rejects.toBe(failure);
    expect(readText).toHaveBeenCalledOnce();
  });

  it("waits for focus to return after permission was granted", async () => {
    const readText = vi.fn()
      .mockRejectedValueOnce(new DOMException("Document is not focused", "NotAllowedError"))
      .mockResolvedValueOnce("clipboard command");
    permissions("granted");
    const hasFocus = vi.spyOn(document, "hasFocus").mockReturnValue(false);
    const focus = vi.fn(() => {
      window.setTimeout(() => {
        hasFocus.mockReturnValue(true);
        window.dispatchEvent(new FocusEvent("focus"));
      }, 0);
    });

    await expect(readTerminalClipboard({ readText }, () => true, focus)).resolves.toBe("clipboard command");
    expect(readText).toHaveBeenCalledTimes(2);
  });
});
