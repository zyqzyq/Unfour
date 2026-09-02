// @vitest-environment jsdom
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import { cleanup, render, screen } from "@testing-library/react";
import {
  FeedbackProvider,
  I18nProvider,
} from "@unfour/ui";
import type { DesktopAppExtensionContext } from "@unfour/app-shell";
import type { AccountContextValue, AccountProfile } from "./features/account/accountTypes";
import type { CloudSyncContextValue } from "./features/cloud-sync/useCloudSync";
import { accountI18nResources } from "./features/account";
import { cloudSyncI18nResources } from "./features/cloud-sync";

const mocks = vi.hoisted(() => ({
  account: null as AccountContextValue | null,
  cloudSync: null as CloudSyncContextValue | null,
}));

vi.mock("./features/account/useAccount", () => ({
  useAccount: () => mocks.account as AccountContextValue,
}));
vi.mock("./features/cloud-sync/useCloudSync", () => ({
  useCloudSync: () => mocks.cloudSync as CloudSyncContextValue,
}));

import { AccountSyncSettings } from "./desktopSettings";

const profile: AccountProfile = {
  id: "account",
  email: "alex@example.com",
  username: "alexchen",
  displayName: "Alex Chen",
  avatarUrl: null,
  entitlements: [{ code: "cloud_sync", status: "active", validUntil: null }],
  devices: [],
};

const extensionContext: DesktopAppExtensionContext = {
  activeTab: { id: "api-main", kind: "api", title: "API Client" },
  activeWorkspace: undefined,
  activateWorkspace: vi.fn(),
  refreshWorkspaces: vi.fn(),
};

function createAccountContext(): AccountContextValue {
  return {
    preview: false,
    state: { kind: "signedIn", profile },
    syncContext: { kind: "ready" },
    overlayOpen: false,
    setOverlayOpen: () => undefined,
    openOverlay: () => undefined,
    signIn: () => undefined,
    signOut: () => undefined,
    retry: () => undefined,
    refreshAccount: async () => undefined,
    refreshing: false,
    setMockState: () => undefined,
  };
}

function createCloudSyncContext(): CloudSyncContextValue {
  return {
    cloudWorkspaceDialogOpen: false,
    detailTarget: null,
    enableTarget: null,
    available: true,
    hasCloudSyncCapability: true,
    errorCode: null,
    globalEnabled: true,
    loading: false,
    revision: 1,
    statuses: new Map(),
    closeCloudWorkspaceDialog: () => undefined,
    closeDetailDialog: () => undefined,
    closeEnableDialog: () => undefined,
    enableWorkspace: async () => undefined,
    openCloudWorkspaceDialog: () => undefined,
    openDetailDialog: () => undefined,
    openEnableDialog: () => undefined,
    pauseWorkspace: async () => undefined,
    refresh: () => undefined,
    refreshNow: async () => undefined,
    retryDeadLetter: async () => undefined,
    retryWorkspace: async () => undefined,
    setServiceEnabled: async () => undefined,
    replaceDeadLetterWithRemote: async () => undefined,
  };
}

function renderSettings() {
  const resources = {
    en: { ...accountI18nResources.en, ...cloudSyncI18nResources.en },
    "zh-CN": {
      ...accountI18nResources["zh-CN"],
      ...cloudSyncI18nResources["zh-CN"],
    },
  };
  return render(
    <I18nProvider
      initialLocale="en"
      resources={resources}
      storageKey="test.desktop-settings.locale"
    >
      <FeedbackProvider>
        <AccountSyncSettings {...extensionContext} />
      </FeedbackProvider>
    </I18nProvider>,
  );
}

beforeEach(() => {
  mocks.account = createAccountContext();
  mocks.cloudSync = createCloudSyncContext();
});

afterEach(cleanup);

describe("Account & Sync settings composition", () => {
  it("renders account identity, entitlement, and cloud sync controls together", () => {
    renderSettings();

    expect(screen.getByRole("heading", { name: "Account & Sync" })).toBeTruthy();
    expect(screen.getByRole("heading", { name: "Account" })).toBeTruthy();
    expect(screen.getByRole("heading", { name: "Cloud Sync" })).toBeTruthy();
    expect(screen.getByText("Signed in")).toBeTruthy();
    expect(screen.getByText("Alex Chen")).toBeTruthy();
    expect(screen.getByText("Unfour Pro")).toBeTruthy();
    expect(screen.getByRole("switch", { name: "Sync service" })).toBeChecked();
  });
});
