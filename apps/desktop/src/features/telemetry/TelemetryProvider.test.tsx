// @vitest-environment jsdom
import { act, cleanup, fireEvent, render, screen, waitFor } from "@testing-library/react";
import { I18nProvider } from "@unfour/ui";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import type { TelemetryPreferences } from "@unfour/command-client";
import { telemetryI18nResources } from "./locales";

const mocks = vi.hoisted(() => ({
  get: vi.fn(),
  mark: vi.fn(),
  record: vi.fn(),
  set: vi.fn(),
}));

vi.mock("./telemetryApi", () => ({
  getTelemetryPreferences: mocks.get,
  markTelemetryNoticeShown: mocks.mark,
  recordTelemetryActive: mocks.record,
  setTelemetryEnabled: mocks.set,
}));
vi.mock("@tauri-apps/plugin-opener", () => ({ openUrl: vi.fn() }));

import { TelemetryNotice } from "./TelemetryNotice";
import { TelemetryProvider } from "./TelemetryProvider";
import { FIRST_NOTICE_GRACE_PERIOD_MS } from "./telemetryPolicy";

let stored: TelemetryPreferences;

function renderNotice() {
  return render(
    <I18nProvider
      initialLocale="en"
      resources={telemetryI18nResources}
      storageKey="test.telemetry.locale"
    >
      <TelemetryProvider>
        <TelemetryNotice />
      </TelemetryProvider>
    </I18nProvider>,
  );
}

beforeEach(() => {
  stored = { enabled: true, noticeShown: false, networkEnabled: true };
  vi.clearAllMocks();
  mocks.get.mockImplementation(async () => ({ ...stored }));
  mocks.mark.mockImplementation(async () => {
    stored = { ...stored, noticeShown: true };
    return { ...stored };
  });
  mocks.set.mockImplementation(async (enabled: boolean) => {
    stored = { ...stored, enabled };
    return { ...stored };
  });
  mocks.record.mockResolvedValue("sent");
});

afterEach(() => {
  cleanup();
  vi.useRealTimers();
});

describe("first telemetry notice", () => {
  it("persists the first display and does not show it on the next mount", async () => {
    const first = renderNotice();
    expect(await screen.findByText("Anonymous usage statistics are enabled")).toBeTruthy();
    await waitFor(() => expect(mocks.mark).toHaveBeenCalledTimes(1));
    first.unmount();

    renderNotice();
    await waitFor(() => expect(mocks.get).toHaveBeenCalledTimes(2));
    await act(async () => undefined);
    expect(screen.queryByText("Anonymous usage statistics are enabled")).toBeNull();
  });

  it("turns telemetry off before the grace period can send", async () => {
    vi.useFakeTimers();
    renderNotice();
    await act(async () => {
      await Promise.resolve();
      await Promise.resolve();
    });

    fireEvent.click(screen.getByRole("button", { name: "Turn off" }));
    await act(async () => {
      await Promise.resolve();
    });
    expect(mocks.set).toHaveBeenCalledWith(false);
    expect(stored.enabled).toBe(false);
    expect(screen.queryByText("Anonymous usage statistics are enabled")).toBeNull();

    await act(() => vi.advanceTimersByTimeAsync(FIRST_NOTICE_GRACE_PERIOD_MS + 1));
    expect(mocks.record).not.toHaveBeenCalled();
  });
});
