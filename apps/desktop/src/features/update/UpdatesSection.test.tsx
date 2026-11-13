// @vitest-environment jsdom
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import { cleanup, fireEvent, render, screen } from "@testing-library/react";
import { I18nProvider } from "@unfour/ui";
import type { UpdateContextValue } from "./updateTypes";
import { updateI18nResources } from "./locales";

const mocks = vi.hoisted(() => ({
  check: vi.fn(),
  context: null as UpdateContextValue | null,
  openDialog: vi.fn(),
}));

vi.mock("./useUpdate", () => ({
  useUpdate: () => mocks.context as UpdateContextValue,
}));

import { UpdatesSection } from "./UpdatesSection";
import { UpdateDialog } from "./UpdateDialog";

function renderUpdates() {
  return render(
    <I18nProvider
      initialLocale="en"
      resources={updateI18nResources}
      storageKey="test.updates-section.locale"
    >
      <UpdatesSection />
    </I18nProvider>,
  );
}

function createContext(
  state: UpdateContextValue["state"],
  distribution: "standard" | "microsoft-store" = "standard",
): UpdateContextValue {
  return {
    meta: {
      name: "Unfour",
      version: "0.9.6",
      distribution,
      channel: "stable",
      commit: null,
      updaterEnabled: distribution === "standard",
      installerExitsApp: false,
      installedVersion: null,
      endpoint: null,
    },
    state,
    dialogOpen: false,
    setDialogOpen: () => undefined,
    openDialog: mocks.openDialog,
    check: mocks.check,
    install: async () => undefined,
    restart: async () => undefined,
  };
}

beforeEach(() => {
  mocks.context = createContext({
    kind: "available",
    info: { version: "0.9.7", currentVersion: "0.9.6", date: null, body: null },
  });
});

afterEach(cleanup);

describe("UpdatesSection", () => {
  it("shows update status and actions without repeating application identity", () => {
    renderUpdates();

    expect(screen.getByRole("heading", { name: "Updates" })).toBeTruthy();
    expect(screen.getByText("Version 0.9.7 is available.")).toBeTruthy();
    expect(screen.getByRole("button", { name: "Check for updates" })).toBeEnabled();
    expect(screen.getByRole("button", { name: "Update available" })).toBeEnabled();
    expect(screen.queryByText("Current version")).toBeNull();
    expect(screen.queryByText("Distribution")).toBeNull();
    expect(screen.queryByText("Update channel")).toBeNull();

    fireEvent.click(screen.getByRole("button", { name: "Check for updates" }));
    expect(mocks.check).toHaveBeenCalledTimes(1);
    fireEvent.click(screen.getByRole("button", { name: "Update available" }));
    expect(mocks.openDialog).toHaveBeenCalledTimes(1);
  });

  it("keeps Microsoft Store updates managed by the Store", () => {
    mocks.context = createContext({ kind: "managedByStore" }, "microsoft-store");
    renderUpdates();

    expect(screen.getByText("Updates are managed by Microsoft Store.")).toBeTruthy();
    expect(screen.queryByRole("button", { name: "Check for updates" })).toBeNull();
  });

  it("reopens the installed update after the dialog is closed", () => {
    mocks.context = createContext({ kind: "installed", info: { version: "0.9.7", currentVersion: "0.9.6", date: null, body: null } });
    renderUpdates();
    expect(screen.getByText("Version 0.9.7 is installed. Restart Unfour to use it.")).toBeVisible();
    fireEvent.click(screen.getByRole("button", { name: "Restart to update" }));
    expect(mocks.openDialog).toHaveBeenCalled();
    expect(screen.queryByRole("button", { name: "Check for updates" })).toBeNull();
  });

  it("lets the user save their work and explicitly restart after installation", () => {
    const restart = vi.fn().mockResolvedValue(undefined);
    mocks.context = {
      ...createContext({ kind: "installed", info: { version: "0.9.7", currentVersion: "0.9.6", date: null, body: null } }),
      dialogOpen: true, restart,
    };
    render(<I18nProvider initialLocale="en" resources={updateI18nResources} storageKey="test.update-dialog">
      <UpdateDialog />
    </I18nProvider>);
    expect(screen.getByText(/Save your work before restarting/)).toBeVisible();
    expect(screen.queryByRole("button", { name: "Download & install" })).toBeNull();
    expect(restart).not.toHaveBeenCalled();
    fireEvent.click(screen.getByRole("button", { name: "Restart now" }));
    expect(restart).toHaveBeenCalledTimes(1);
  });
});
