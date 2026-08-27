// @vitest-environment jsdom
import "@testing-library/jest-dom/vitest";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import { cleanup, render, screen, waitFor } from "@testing-library/react";
import type { DesktopAppExtensions } from "@unfour/app-shell";

const mocks = vi.hoisted(() => ({
  extensions: undefined as DesktopAppExtensions | undefined,
  getAccountState: vi.fn(),
}));

vi.mock("@unfour/app-shell", () => ({
  DesktopApp: ({ extensions }: { extensions?: DesktopAppExtensions }) => {
    mocks.extensions = extensions;
    return <div data-testid="desktop-app">local desktop</div>;
  },
}));

vi.mock("@unfour/ui", async (importOriginal) => ({
  ...await importOriginal<typeof import("@unfour/ui")>(),
  useI18n: () => ({ t: (key: string) => key }),
}));

vi.mock("./features/account/accountApi", () => ({
  beginAccountSignIn: vi.fn(),
  getAccountState: mocks.getAccountState,
  isTauriRuntime: () => true,
  listenForAccountDeepLinks: vi.fn(async () => vi.fn()),
  signOutAccount: vi.fn(),
}));

vi.mock("./features/account/accountForeground", () => ({
  listenForAccountForeground: vi.fn(async () => vi.fn()),
}));

vi.mock("./features/cloud-sync/syncApi", () => ({
  disableCloudSync: vi.fn(),
  enableCloudSync: vi.fn(),
  getCloudSyncStatus: vi.fn(),
  getGlobalSyncEnabled: vi.fn(),
  getLocalWorkspaces: vi.fn(),
  retryDeadLetterCurrentLocal: vi.fn(),
  setGlobalSyncEnabled: vi.fn(),
  syncErrorCode: () => "cloud_sync_failed",
  syncNow: vi.fn(),
  useRemoteDeadLetter: vi.fn(),
}));

import App from "./App";

beforeEach(() => {
  vi.clearAllMocks();
  mocks.extensions = undefined;
  mocks.getAccountState.mockRejectedValue(new Error("account command unavailable"));
});

afterEach(cleanup);

describe("desktop feature composition", () => {
  it("injects Account and Cloud Sync extensions without blocking the core desktop", async () => {
    render(<App />);

    expect(screen.getByTestId("desktop-app")).toHaveTextContent("local desktop");
    await waitFor(() => expect(mocks.getAccountState).toHaveBeenCalledTimes(1));
    expect(screen.getByTestId("desktop-app")).toBeInTheDocument();

    const extensions = mocks.extensions;
    expect(extensions?.settingsSections?.map((section) => section.id)).toEqual([
      "account.settings",
      "cloud-sync.settings",
    ]);
    expect(extensions?.titleBarEnd).toBeTypeOf("function");
    expect(extensions?.workspaceDecoration).toBeTypeOf("function");
    expect(extensions?.workspaceMenuActions).toBeTypeOf("function");
    expect(extensions?.workspaceMenuFooterActions?.[0]).toMatchObject({
      disabled: true,
      id: "cloud-sync.open-cloud-workspace",
      label: "cloudSync.openCloudWorkspace",
    });
    expect(extensions?.overlays).toBeTypeOf("function");
  });
});
