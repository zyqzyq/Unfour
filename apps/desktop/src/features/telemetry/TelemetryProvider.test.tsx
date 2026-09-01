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
import { PrivacySection } from "./PrivacySection";
import { FIRST_NOTICE_GRACE_PERIOD_MS } from "./telemetryPolicy";

let stored: TelemetryPreferences;

function renderTelemetrySurface({ includeNotice = true, includeSettings = false } = {}) {
  return render(
    <I18nProvider
      initialLocale="en"
      resources={telemetryI18nResources}
      storageKey="test.telemetry.locale"
    >
      <TelemetryProvider>
        {includeNotice && <TelemetryNotice />}
        {includeSettings && <PrivacySection />}
      </TelemetryProvider>
    </I18nProvider>,
  );
}

function renderNotice() {
  return renderTelemetrySurface();
}

async function flushTelemetryPromises() {
  await act(async () => {
    await Promise.resolve();
    await Promise.resolve();
    await Promise.resolve();
  });
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
  it("marks the notice after it is mounted and does not show it on the next mount", async () => {
    const providerOnly = renderTelemetrySurface({ includeNotice: false });
    await waitFor(() => expect(mocks.get).toHaveBeenCalledTimes(1));
    expect(mocks.mark).not.toHaveBeenCalled();
    providerOnly.unmount();

    const first = renderNotice();
    expect(await screen.findByText("Anonymous usage statistics are enabled")).toBeTruthy();
    await waitFor(() => expect(mocks.mark).toHaveBeenCalledTimes(1));
    first.unmount();

    renderNotice();
    await waitFor(() => expect(mocks.get).toHaveBeenCalledTimes(3));
    await act(async () => undefined);
    expect(screen.queryByText("Anonymous usage statistics are enabled")).toBeNull();
  });

  it("turns telemetry off before the grace period can send when persistence succeeds", async () => {
    vi.useFakeTimers();
    renderNotice();
    await flushTelemetryPromises();

    fireEvent.click(screen.getByRole("button", { name: "Turn off" }));
    await flushTelemetryPromises();
    expect(mocks.set).toHaveBeenCalledWith(false);
    expect(stored.enabled).toBe(false);
    expect(screen.queryByText("Anonymous usage statistics are enabled")).toBeNull();

    await act(() => vi.advanceTimersByTimeAsync(FIRST_NOTICE_GRACE_PERIOD_MS + 1));
    expect(mocks.record).not.toHaveBeenCalled();
  });

  it("keeps the current session suppressed when disabling fails", async () => {
    vi.useFakeTimers();
    mocks.set.mockRejectedValueOnce(new Error("storage unavailable"));
    renderTelemetrySurface({ includeSettings: true });
    await flushTelemetryPromises();

    fireEvent.click(screen.getByRole("button", { name: "Turn off" }));
    await flushTelemetryPromises();
    expect(stored.enabled).toBe(true);
    expect(screen.getByText("The privacy preference could not be saved. Try again.")).toBeTruthy();

    await act(() => vi.advanceTimersByTimeAsync(FIRST_NOTICE_GRACE_PERIOD_MS + 1));
    expect(mocks.record).not.toHaveBeenCalled();
  });

  it("allows an explicit re-enable to clear session suppression", async () => {
    vi.useFakeTimers();
    renderTelemetrySurface({ includeSettings: true });
    await flushTelemetryPromises();

    fireEvent.click(screen.getByRole("button", { name: "Turn off" }));
    await flushTelemetryPromises();
    const toggle = screen.getByRole("switch", { name: "Anonymous usage statistics" });
    expect(toggle).not.toBeChecked();

    fireEvent.click(toggle);
    await flushTelemetryPromises();
    expect(mocks.set).toHaveBeenNthCalledWith(2, true);

    await act(() => vi.advanceTimersByTimeAsync(FIRST_NOTICE_GRACE_PERIOD_MS + 1));
    expect(mocks.record).toHaveBeenCalledTimes(1);
  });

  it("does not mark or schedule telemetry for a network-disabled build", async () => {
    vi.useFakeTimers();
    stored.networkEnabled = false;
    renderTelemetrySurface({ includeSettings: true });
    await flushTelemetryPromises();

    expect(screen.queryByText("Anonymous usage statistics are enabled")).toBeNull();
    expect(screen.getByText("Network sending is disabled for this Test build, regardless of this preference.")).toBeTruthy();
    expect(mocks.mark).not.toHaveBeenCalled();

    await act(() => vi.advanceTimersByTimeAsync(FIRST_NOTICE_GRACE_PERIOD_MS + 1));
    expect(mocks.record).not.toHaveBeenCalled();
  });
});
