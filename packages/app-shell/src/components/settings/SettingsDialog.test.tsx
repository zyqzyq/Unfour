// @vitest-environment jsdom
import type { ReactNode } from "react";
import { afterEach, describe, expect, it, vi } from "vitest";
import { cleanup, fireEvent, render, screen, within } from "@testing-library/react";
import { I18nProvider } from "@unfour/ui";
import { SettingsDialog } from "./SettingsDialog";
import type { DesktopAppExtensionContext, DesktopAppSettingsSection } from "../../extensions";

vi.mock("./SettingsGeneral", () => ({
  SettingsGeneral: ({ children }: { children?: ReactNode }) => (
    <div data-testid="general-page">{children}</div>
  ),
}));
vi.mock("./SettingsMcp", () => ({
  SettingsMcp: () => <div data-testid="mcp-page">MCP settings</div>,
}));
vi.mock("./SettingsAbout", () => ({
  SettingsAbout: ({ children }: { children?: ReactNode }) => (
    <div data-testid="about-page">{children}</div>
  ),
}));

const extensionContext: DesktopAppExtensionContext = {
  activeTab: { id: "api-main", kind: "api", title: "API Client" },
  activeWorkspace: undefined,
  activateWorkspace: vi.fn(),
  refreshWorkspaces: vi.fn(),
};

const settingsSections: DesktopAppSettingsSection[] = [
  {
    id: "telemetry.privacy",
    component: () => <span>Privacy controls</span>,
    slot: "general",
  },
  {
    id: "account-sync.settings",
    component: () => <span>Account and sync controls</span>,
    label: "Account & Sync",
  },
  {
    id: "updates.settings",
    component: () => <span>Update controls</span>,
    slot: "about",
  },
];

function renderSettings() {
  return render(
    <I18nProvider initialLocale="en" storageKey="test.settings-dialog.locale">
      <SettingsDialog
        extensionContext={extensionContext}
        extensionSections={settingsSections}
        onOpenChange={vi.fn()}
        open
      />
    </I18nProvider>,
  );
}

afterEach(cleanup);

describe("SettingsDialog information architecture", () => {
  it("shows only the four core navigation items and embeds feature sections", () => {
    renderSettings();

    const navigation = screen.getByRole("navigation", { name: "Settings sections" });
    expect(within(navigation).getAllByRole("button").map((button) => button.textContent)).toEqual([
      "General",
      "MCP",
      "Account & Sync",
      "About",
    ]);
    for (const label of ["Privacy", "Account", "Cloud Sync", "Updates"]) {
      expect(within(navigation).queryByRole("button", { name: label })).toBeNull();
    }

    expect(screen.getByText("Privacy controls")).toBeTruthy();

    fireEvent.click(within(navigation).getByRole("button", { name: "Account & Sync" }));
    expect(screen.getByText("Account and sync controls")).toBeTruthy();

    fireEvent.click(within(navigation).getByRole("button", { name: "About" }));
    expect(screen.getByText("Update controls")).toBeTruthy();

    fireEvent.click(within(navigation).getByRole("button", { name: "MCP" }));
    expect(screen.getByTestId("mcp-page")).toBeTruthy();
  });
});
