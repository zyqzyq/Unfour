// @vitest-environment jsdom
import type { ReactNode } from "react";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import { cleanup, render, screen, waitFor } from "@testing-library/react";
import { I18nProvider } from "@unfour/ui";
import { SettingsAbout } from "./SettingsAbout";

const getAppInfo = vi.hoisted(() => vi.fn());

vi.mock("@unfour/command-client", () => ({ getAppInfo }));

function renderAbout(children?: ReactNode) {
  return render(
    <I18nProvider initialLocale="en" storageKey="test.settings-about.locale">
      <SettingsAbout>{children}</SettingsAbout>
    </I18nProvider>,
  );
}

beforeEach(() => {
  getAppInfo.mockResolvedValue({
    name: "Unfour",
    version: "0.9.3",
    distribution: "microsoft-store",
    channel: "stable",
    commit: "0123456789abcdef-dirty",
  });
});

afterEach(() => {
  cleanup();
  vi.clearAllMocks();
});

describe("SettingsAbout", () => {
  it("keeps application identity and embedded update state on one About page", async () => {
    renderAbout(<div>Update available</div>);

    expect(screen.getByRole("heading", { name: "About" })).toBeTruthy();
    expect(screen.getByRole("heading", { name: "Application" })).toBeTruthy();
    expect(screen.getByText("Update available")).toBeTruthy();
    expect(screen.getByRole("link", { name: /unfour\.dev/i })).toBeTruthy();
    expect(screen.getByRole("link", { name: /github\.com/i })).toBeTruthy();
    expect(screen.queryByRole("heading", { name: "Links" })).toBeNull();
    expect(screen.getByRole("heading", { name: "Actions" })).toBeTruthy();

    await waitFor(() => {
      expect(screen.getByText("0.9.3")).toBeTruthy();
      expect(screen.getByText("Microsoft Store")).toBeTruthy();
      expect(screen.getByText("Stable")).toBeTruthy();
      expect(screen.getByText("0123456789ab-dirty")).toBeTruthy();
    });
  });
});
