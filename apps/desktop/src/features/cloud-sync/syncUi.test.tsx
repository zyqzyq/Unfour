// @vitest-environment jsdom
import type { Workspace } from "@unfour/command-client";
import type { DesktopAppExtensionContext } from "@unfour/app-shell";
import type { ButtonHTMLAttributes, ReactNode } from "react";
import { StrictMode } from "react";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import { act, cleanup, fireEvent, render, screen, waitFor } from "@testing-library/react";

const mocks = vi.hoisted(() => ({
  context: null as unknown as Record<string, unknown>,
  download: vi.fn(),
  feedbackSuccess: vi.fn(),
  keepLocal: vi.fn(),
  listCloud: vi.fn(),
  listConflicts: vi.fn(),
  retryDeadLetter: vi.fn(),
  replaceDeadLetterWithRemote: vi.fn(),
  useRemote: vi.fn(),
}));

const translations: Record<string, string> = {
  "cloudSync.enableDialog.title": "Enable Cloud Sync for Backend?",
  "cloudSync.enableDialog.willSync": "Will sync",
  "cloudSync.enableDialog.willNotSync": "Will not sync",
  "cloudSync.scope.workspace": "Workspace settings",
  "cloudSync.scope.connections": "Connection definitions",
  "cloudSync.scope.environments": "Environments",
  "cloudSync.scope.variables": "Non-secret variables",
  "cloudSync.scope.secrets": "Secret values",
  "cloudSync.scope.apiCollections": "API collections",
  "cloudSync.scope.apiFolders": "API folders",
  "cloudSync.scope.apiRequests": "API requests",
  "cloudSync.scope.ssh": "SSH connections",
  "cloudSync.scope.sshTasks": "SSH tasks",
  "cloudSync.scope.database": "Database connections",
  "cloudSync.scope.historyRuntime": "History and runtime results",
  "cloudSync.secretPolicy": "Secret values are never uploaded to Cloud Sync.",
  "cloudSync.cancel": "Cancel",
  "cloudSync.enableAndUpload": "Enable & Upload",
  "cloudSync.close": "Close",
  "cloudSync.cloudDialog.title": "Open Cloud Workspace",
  "cloudSync.cloudDialog.description": "Download a workspace",
  "cloudSync.cloudDialog.empty": "No cloud workspaces",
  "cloudSync.cloudDialog.updated": "Updated now",
  "cloudSync.cloudDialog.downloadAndOpen": "Download & Open",
  "cloudSync.cloudDialog.downloaded": "Workspace downloaded",
  "cloudSync.cloudDialog.secretReminder": "Secret reminder",
  "cloudSync.openCloudWorkspace": "Open Cloud Workspace",
  "cloudSync.contextUnavailable": "Cloud Sync unavailable",
  "cloudSync.contextUnavailableDescription": "Account is current, but sync is unavailable.",
  "cloudSync.errors.invalidData": "The cloud could not recognize this local change.",
  "cloudSync.status.synced": "Synced",
  "cloudSync.status.syncing": "Syncing",
  "cloudSync.status.paused": "Paused",
  "cloudSync.status.auth_required": "Sign-in required",
  "cloudSync.status.capability_required": "Cloud Sync plan required",
  "cloudSync.status.attention": "Needs attention",
  "cloudSync.pending": "Pending",
  "cloudSync.retry": "Retry",
  "cloudSync.never": "Never",
  "cloudSync.detail.status": "Status",
  "cloudSync.detail.lastSynced": "Last synced",
  "cloudSync.detail.changesPending": "Changes pending",
  "cloudSync.detail.authRequiredDescription": "Sign in again to continue syncing.",
  "cloudSync.detail.capabilityRequiredDescription": "Upgrade the plan to resume syncing.",
  "cloudSync.advancedDiagnostics": "Advanced diagnostics",
  "cloudSync.conflict.variableTitle": "Variable conflict",
  "cloudSync.conflict.apiTitle": "API conflict",
  "cloudSync.conflict.connectionTitle": "Connection conflict",
  "cloudSync.conflict.thisDevice": "This device",
  "cloudSync.conflict.cloud": "Cloud",
  "cloudSync.conflict.useThisDevice": "Use this device",
  "cloudSync.conflict.useCloud": "Use cloud",
  "cloudSync.technicalDetails": "Technical details",
  "cloudSync.deadLetter.title": "Changes requiring recovery",
  "cloudSync.deadLetter.description": "Permanently rejected changes",
  "cloudSync.deadLetter.count": "Blocked changes",
  "cloudSync.deadLetter.retryCurrentLocal": "Retry current local",
  "cloudSync.deadLetter.useRemote": "Use cloud version",
  "cloudSync.deadLetter.confirmUseRemote": "Discard local change",
  "cloudSync.deadLetter.confirmTitle": "Use cloud version?",
  "cloudSync.deadLetter.confirmDescription": "This cannot be undone.",
  "cloudSync.deadLetter.entityType.workspaceVariable": "Workspace variable",
  "cloudSync.deadLetter.entityType.connection": "Connection",
  "cloudSync.deadLetter.entityType.apiCollection": "API collection",
  "cloudSync.deadLetter.entityType.apiFolder": "API folder",
  "cloudSync.deadLetter.entityType.apiRequest": "API request",
  "cloudSync.deadLetter.entityType.sshTask": "SSH task",
  "cloudSync.deadLetter.entityType.sshTaskStep": "SSH task step",
};

vi.mock("@unfour/ui", () => ({
  Button: ({ children, ...props }: { children: ReactNode } & ButtonHTMLAttributes<HTMLButtonElement>) => <button {...props}>{children}</button>,
  ConfirmDialog: ({ confirmLabel, description, onConfirm, onOpenChange, open, title }: { confirmLabel: string; description?: ReactNode; onConfirm(): void; onOpenChange(open: boolean): void; open: boolean; title: string }) => open ? <div aria-label={title} role="dialog"><div>{description}</div><button onClick={() => onOpenChange(false)}>Cancel recovery</button><button onClick={onConfirm}>{confirmLabel}</button></div> : null,
  Dialog: ({ children, open }: { children: ReactNode; open: boolean }) => open ? <>{children}</> : null,
  DialogBody: ({ children }: { children: ReactNode }) => <div>{children}</div>,
  DialogContent: ({ children, title }: { children: ReactNode; title: string }) => <div aria-label={title} role="dialog">{children}</div>,
  DialogFooter: ({ children }: { children: ReactNode }) => <div>{children}</div>,
  DialogHeader: ({ children }: { children: ReactNode }) => <div>{children}</div>,
  DialogTitle: ({ children }: { children: ReactNode }) => <h2>{children}</h2>,
  DialogXClose: () => null,
  EmptyState: ({ children }: { children: ReactNode }) => <div>{children}</div>,
  ErrorState: ({ children }: { children: ReactNode }) => <div>{children}</div>,
  LoadingState: () => <div>Loading</div>,
  StatusBadge: ({ children }: { children: ReactNode }) => <span>{children}</span>,
  useFeedback: () => ({ success: mocks.feedbackSuccess }),
  useI18n: () => ({ t: (key: string) => translations[key] ?? key }),
}));

vi.mock("./useCloudSync", () => ({ useCloudSync: () => mocks.context }));
vi.mock("../account/useAccount", () => ({ useAccount: () => ({ state: { kind: "signedIn", profile: { displayName: "Reid", username: null, email: "reid@example.com" } } }) }));
vi.mock("./syncApi", () => ({
  downloadCloudWorkspace: mocks.download,
  getSyncDiagnostics: vi.fn(),
  keepLocalConflict: mocks.keepLocal,
  listCloudWorkspaces: mocks.listCloud,
  listSyncConflicts: mocks.listConflicts,
  syncErrorCode: () => "cloud_sync_failed",
  useRemoteConflict: mocks.useRemote,
}));

import { CloudWorkspaceDialog } from "./CloudWorkspaceDialog";
import { EnableCloudSyncDialog } from "./EnableCloudSyncDialog";
import { CloudSyncSection } from "./CloudSyncSection";
import { CloudSyncWorkspaceDecoration } from "./CloudSyncWorkspaceDecoration";
import { SyncConflictList } from "./SyncConflictList";
import { WorkspaceSyncDialog } from "./WorkspaceSyncDialog";

const emptyStatus = { binding: null, pendingCount: 0, uncertainCount: 0, inFlightCount: 0, deadCount: 0, deadLetters: [], conflictCount: 0, running: false };
const workspace: Workspace = { id: "workspace", name: "Backend", environmentType: "dev", mcpPolicy: "auto", isDefault: false, lastOpenedAt: null, createdAt: "", updatedAt: "", deletedAt: null, revision: 1 };
const extensionContext: DesktopAppExtensionContext = { activeWorkspace: workspace, activeTab: { id: "api", kind: "api", title: "API" }, activateWorkspace: vi.fn(), refreshWorkspaces: vi.fn() };

function baseContext() {
  return {
    cloudWorkspaceDialogOpen: false, detailTarget: null, enableTarget: null, available: true, hasCloudSyncCapability: true, errorCode: null,
    globalEnabled: true, loading: false, statuses: new Map([[workspace.id, emptyStatus]]), workspaceErrors: new Map(),
    closeCloudWorkspaceDialog: vi.fn(), closeDetailDialog: vi.fn(), closeEnableDialog: vi.fn(), enableWorkspace: vi.fn().mockResolvedValue(undefined),
    openCloudWorkspaceDialog: vi.fn(), openDetailDialog: vi.fn(), openEnableDialog: vi.fn(), pauseWorkspace: vi.fn(),
    refreshNow: vi.fn().mockResolvedValue(undefined), replaceDeadLetterWithRemote: mocks.replaceDeadLetterWithRemote, retryDeadLetter: mocks.retryDeadLetter, retryWorkspace: vi.fn(), setServiceEnabled: vi.fn(),
  };
}

beforeEach(() => {
  vi.clearAllMocks();
  mocks.context = baseContext();
  mocks.listCloud.mockResolvedValue([]);
  mocks.listConflicts.mockResolvedValue([]);
  mocks.retryDeadLetter.mockResolvedValue(undefined);
  mocks.replaceDeadLetterWithRemote.mockResolvedValue(undefined);
});
afterEach(cleanup);

describe("Cloud Sync request lifecycles", () => {
  it("drops a recovery confirmation when the detail target changes", () => {
    mocks.context = {
      ...baseContext(), detailTarget: { id: "old", name: "Old" },
      statuses: new Map([["old", { ...emptyStatus, deadCount: 1, deadLetters: [{
        operationId: "old-operation", entityId: "variable", entityName: "Old variable", entityType: "workspaceVariable", errorCode: "invalid_sync_entity",
      }] }]]),
    };
    const { rerender } = render(<WorkspaceSyncDialog />);
    fireEvent.click(screen.getByText("Use cloud version"));
    expect(screen.getByText("Discard local change")).toBeTruthy();
    mocks.context = { ...mocks.context, detailTarget: { id: "new", name: "New" } };
    rerender(<WorkspaceSyncDialog />);
    expect(screen.queryByText("Discard local change")).toBeNull();
    expect(mocks.replaceDeadLetterWithRemote).not.toHaveBeenCalled();
  });

  it("coalesces StrictMode mount reads without losing the live subscription", async () => {
    mocks.context = { ...baseContext(), cloudWorkspaceDialogOpen: true };
    render(<StrictMode><CloudWorkspaceDialog {...extensionContext} /><SyncConflictList workspaceId="workspace" onResolved={() => {}} /></StrictMode>);
    await screen.findByText("No cloud workspaces");
    expect(mocks.listCloud).toHaveBeenCalledTimes(1);
    expect(mocks.listConflicts).toHaveBeenCalledExactlyOnceWith("workspace");
    expect(screen.queryByText("Loading")).toBeNull();
  });

  it("filters updated local bindings without refetching the remote workspace list", async () => {
    const cloud = { cloudWorkspaceId: "cloud", name: "Remote", updatedAt: "2026-01-01" };
    mocks.context = { ...baseContext(), cloudWorkspaceDialogOpen: true };
    mocks.listCloud.mockResolvedValue([cloud]);
    const { rerender } = render(<CloudWorkspaceDialog {...extensionContext} />);
    await screen.findByText("Remote");
    mocks.context = { ...mocks.context, statuses: new Map([[workspace.id, { ...emptyStatus, binding: { cloudWorkspaceId: "cloud" } }]]) };
    rerender(<CloudWorkspaceDialog {...extensionContext} />);
    expect(screen.getByText("No cloud workspaces")).toBeTruthy();
    expect(mocks.listCloud).toHaveBeenCalledTimes(1);
    mocks.context = { ...mocks.context, cloudWorkspaceDialogOpen: false };
    rerender(<CloudWorkspaceDialog {...extensionContext} />);
    mocks.context = { ...mocks.context, cloudWorkspaceDialogOpen: true };
    rerender(<CloudWorkspaceDialog {...extensionContext} />);
    await screen.findByText("No cloud workspaces");
    expect(mocks.listCloud).toHaveBeenCalledTimes(2);
  });

  it("discards a cloud listing completed after the dialog was closed and reopened", async () => {
    let complete!: (items: unknown[]) => void;
    mocks.listCloud.mockReturnValueOnce(new Promise((resolve) => { complete = resolve; }));
    mocks.context = { ...baseContext(), cloudWorkspaceDialogOpen: true };
    const { rerender } = render(<CloudWorkspaceDialog {...extensionContext} />);
    mocks.context = { ...mocks.context, cloudWorkspaceDialogOpen: false };
    rerender(<CloudWorkspaceDialog {...extensionContext} />);
    mocks.context = { ...mocks.context, cloudWorkspaceDialogOpen: true };
    rerender(<CloudWorkspaceDialog {...extensionContext} />);
    await screen.findByText("No cloud workspaces");
    await act(async () => { complete([{ cloudWorkspaceId: "old", name: "Old workspace", updatedAt: "2026-01-01" }]); });
    expect(screen.queryByText("Old workspace")).toBeNull();
    expect(mocks.listCloud).toHaveBeenCalledTimes(2);
  });

  it("does not reload conflicts for callback identity changes or apply an old workspace result", async () => {
    let complete!: (items: unknown[]) => void;
    mocks.listConflicts.mockReturnValueOnce(new Promise((resolve) => { complete = resolve; }));
    const { rerender } = render(<SyncConflictList onResolved={() => {}} workspaceId="old" />);
    rerender(<SyncConflictList onResolved={() => {}} workspaceId="old" />);
    expect(mocks.listConflicts).toHaveBeenCalledTimes(1);
    rerender(<SyncConflictList onResolved={() => {}} workspaceId="new" />);
    await act(async () => {});
    await act(async () => { complete([{ entityId: "old-item", entityType: "workspaceVariable", operation: "update", localPayload: { name: "Stale conflict" }, remotePayload: null }]); });
    expect(screen.queryByText("Stale conflict")).toBeNull();
    expect(mocks.listConflicts.mock.calls).toEqual([["old"], ["new"]]);
  });
});

describe("Cloud Sync UI", () => {
  it("does not decorate a local-only workspace", () => {
    const { container } = render(<CloudSyncWorkspaceDecoration {...extensionContext} active placement="listItem" workspace={workspace} />);
    expect(container.firstChild).toBeNull();
  });

  it("renders workspace decorations for synced, syncing, and attention states", () => {
    for (const [label, status] of [
      ["Synced", { ...emptyStatus, binding: { syncEnabled: true, state: "active", initialConfirmed: 0, initialTotal: 0, lastError: null } }],
      ["Syncing", { ...emptyStatus, binding: { syncEnabled: true, state: "active", initialConfirmed: 0, initialTotal: 0, lastError: null }, running: true }],
      ["Needs attention", { ...emptyStatus, binding: { syncEnabled: true, state: "conflict", initialConfirmed: 0, initialTotal: 0, lastError: null }, conflictCount: 1 }],
    ] as const) {
      mocks.context = { ...baseContext(), statuses: new Map([[workspace.id, status]]) };
      const { unmount } = render(<CloudSyncWorkspaceDecoration {...extensionContext} active placement="listItem" workspace={workspace} />);
      expect(screen.getByRole("button").getAttribute("aria-label")).toBe(label);
      unmount();
    }
  });

  it("keeps a workspace status failure visible as an attention entry", () => {
    mocks.context = {
      ...baseContext(),
      statuses: new Map(),
      workspaceErrors: new Map([[workspace.id, "cloud_sync_storage_failed"]]),
    };
    render(<CloudSyncWorkspaceDecoration {...extensionContext} active placement="listItem" workspace={workspace} />);
    expect(screen.getByRole("button").getAttribute("aria-label")).toBe("Needs attention");
  });

  it("explains scope and only enables after confirmation", async () => {
    mocks.context = { ...baseContext(), enableTarget: { id: "workspace", name: "Backend" } };
    render(<EnableCloudSyncDialog />);
    expect(screen.getByText("Workspace settings")).toBeTruthy();
    expect(screen.getByText("Connection definitions")).toBeTruthy();
    expect(screen.getByText("Secret values are never uploaded to Cloud Sync.")).toBeTruthy();
    fireEvent.click(screen.getByText("Cancel"));
    expect(mocks.context.enableWorkspace).not.toHaveBeenCalled();
    const confirm = screen.getByText("Enable & Upload");
    fireEvent.click(confirm);
    fireEvent.click(confirm);
    await waitFor(() => expect(mocks.context.enableWorkspace).toHaveBeenCalledTimes(1));
  });

  it("shows empty, error-free cloud state and downloads then opens a workspace", async () => {
    mocks.context = { ...baseContext(), cloudWorkspaceDialogOpen: true };
    const cloud = { cloudWorkspaceId: "cloud", rootEntityId: "remote", name: "Remote", currentCursor: 1, createdAt: "2026-01-01", updatedAt: "2026-01-01" };
    mocks.listCloud.mockResolvedValue([cloud]);
    mocks.download.mockResolvedValue("downloaded");
    render(<CloudWorkspaceDialog {...extensionContext} />);
    fireEvent.click(await screen.findByText("Download & Open"));
    await waitFor(() => expect(extensionContext.refreshWorkspaces).toHaveBeenCalled());
    expect(extensionContext.activateWorkspace).toHaveBeenCalledWith("downloaded");
    expect(mocks.feedbackSuccess).toHaveBeenCalled();
  });

  it("surfaces cloud workspace API errors and allows retry", async () => {
    mocks.context = { ...baseContext(), cloudWorkspaceDialogOpen: true };
    mocks.listCloud.mockRejectedValueOnce(new Error("offline"));
    render(<CloudWorkspaceDialog {...extensionContext} />);
    expect(await screen.findByText("cloudSync.errors.generic")).toBeTruthy();
    expect(screen.getByText("Retry")).toBeTruthy();
  });

  it("shows a real empty cloud workspace state", async () => {
    mocks.context = { ...baseContext(), cloudWorkspaceDialogOpen: true };
    render(<CloudWorkspaceDialog {...extensionContext} />);
    expect(await screen.findByText("No cloud workspaces")).toBeTruthy();
  });

  it("keeps workspace management actions out of Settings", () => {
    render(<CloudSyncSection {...extensionContext} />);
    expect(screen.queryByText("Enable All")).toBeNull();
    expect(screen.queryByText("Pause All")).toBeNull();
    expect(screen.queryByText("Sync All Now")).toBeNull();
    expect(screen.getByText("Open Cloud Workspace")).toBeTruthy();
  });

  it("shows local sync failure separately from the current paid account", () => {
    mocks.context = {
      ...baseContext(),
      available: false,
      errorCode: "cloud_sync_storage_failed",
      statuses: new Map(),
    };
    render(<CloudSyncSection {...extensionContext} />);
    expect(screen.getByText("Cloud Sync unavailable")).toBeTruthy();
    expect(screen.getByText("cloudSync.errors.storage")).toBeTruthy();
    expect(screen.queryByRole("switch")).toBeNull();
  });

  it("keeps pending changes visible while a workspace is paused", () => {
    mocks.context = {
      ...baseContext(),
      detailTarget: { id: workspace.id, name: workspace.name },
      statuses: new Map([[workspace.id, {
        ...emptyStatus,
        binding: {
          accountId: "account",
          localWorkspaceId: workspace.id,
          cloudWorkspaceId: "cloud",
          lastPulledCursor: 1,
          syncEnabled: false,
          state: "paused",
          initialCursor: 0,
          initialTotal: 1,
          initialConfirmed: 1,
          initializationCheckpoint: null,
          generation: 1,
          lastSuccessAt: null,
          lastError: null,
          consecutiveFailureCount: 0,
        },
        pendingCount: 2,
      }]]),
    };
    render(<WorkspaceSyncDialog />);
    expect(screen.getByText("Pending")).toBeTruthy();
    expect(screen.getByText("Changes pending")).toBeTruthy();
  });

  it("offers a retry when a workspace is blocked by an expired session", async () => {
    mocks.context = {
      ...baseContext(),
      detailTarget: { id: workspace.id, name: workspace.name },
      statuses: new Map([[workspace.id, {
        ...emptyStatus,
        binding: {
          accountId: "account", localWorkspaceId: workspace.id, cloudWorkspaceId: "cloud",
          lastPulledCursor: 1, syncEnabled: true, state: "error", initialCursor: 0,
          initialTotal: 1, initialConfirmed: 1, initializationCheckpoint: null, generation: 1,
          lastSuccessAt: null, lastError: "cloud_sync_unauthorized", consecutiveFailureCount: 1,
        },
        pendingCount: 1,
      }]]),
    };
    render(<WorkspaceSyncDialog />);
    expect(screen.getByText("Sign-in required")).toBeTruthy();
    expect(screen.getByText("Sign in again to continue syncing.")).toBeTruthy();
    fireEvent.click(screen.getByText("Retry"));
    await waitFor(() => expect(mocks.context.retryWorkspace).toHaveBeenCalledWith("workspace"));
  });

  it("shows entitlement recovery without asking the user to sign in", () => {
    mocks.context = {
      ...baseContext(),
      detailTarget: { id: workspace.id, name: workspace.name },
      statuses: new Map([[workspace.id, {
        ...emptyStatus,
        binding: {
          accountId: "account", localWorkspaceId: workspace.id, cloudWorkspaceId: "cloud",
          lastPulledCursor: 1, syncEnabled: true, state: "error", initialCursor: 0,
          initialTotal: 1, initialConfirmed: 1, initializationCheckpoint: null, generation: 1,
          lastSuccessAt: null, lastError: "cloud_sync_entitlement_required", consecutiveFailureCount: 1,
        },
        pendingCount: 1,
      }]]),
    };
    render(<WorkspaceSyncDialog />);
    expect(screen.getByText("Cloud Sync plan required")).toBeTruthy();
    expect(screen.getByText("Upgrade the plan to resume syncing.")).toBeTruthy();
    expect(screen.queryByText("Sign-in required")).toBeNull();
  });

  it("shows dead-letter details, counts them as pending, and confirms use-remote", async () => {
    mocks.context = {
      ...baseContext(),
      detailTarget: { id: workspace.id, name: workspace.name },
      statuses: new Map([[workspace.id, {
        ...emptyStatus,
        binding: {
          accountId: "account", localWorkspaceId: workspace.id, cloudWorkspaceId: "cloud",
          lastPulledCursor: 1, syncEnabled: true, state: "error", initialCursor: 0,
          initialTotal: 1, initialConfirmed: 1, initializationCheckpoint: null, generation: 1,
          lastSuccessAt: null, lastError: "cloud_sync_dead_letter_blocked", consecutiveFailureCount: 1,
        },
        deadCount: 1,
        deadLetters: [{
          operationId: "old-operation", entityType: "workspaceVariable", entityId: "variable",
          entityName: "API_HOST", errorCode: "invalid_sync_entity",
        }],
      }]]),
    };
    render(<WorkspaceSyncDialog />);
    expect(screen.getByText("API_HOST")).toBeTruthy();
    expect(screen.getByText("Workspace variable")).toBeTruthy();
    expect(screen.getByText("The cloud could not recognize this local change.")).toBeTruthy();
    expect(screen.getByText("invalid_sync_entity")).toBeTruthy();
    expect(screen.getByText("Changes pending")).toBeTruthy();
    expect(screen.getByText("Blocked changes")).toBeTruthy();

    fireEvent.click(screen.getByText("Retry current local"));
    await waitFor(() => expect(mocks.retryDeadLetter).toHaveBeenCalledWith("workspace", "old-operation"));

    fireEvent.click(screen.getByText("Use cloud version"));
    expect(mocks.replaceDeadLetterWithRemote).not.toHaveBeenCalled();
    fireEvent.click(screen.getByText("Discard local change"));
    await waitFor(() => expect(mocks.replaceDeadLetterWithRemote).toHaveBeenCalledWith("workspace", "old-operation"));
  });

  it("presents variable conflict values and resolves either side", async () => {
    mocks.listConflicts.mockResolvedValue([{ cloudWorkspaceId: "cloud", entityType: "workspaceVariable", entityId: "var", serverVersion: 1, operation: "upsert", localPayload: { name: "API_HOST", value: "http://localhost" }, remotePayload: { name: "API_HOST", value: "https://cloud" }, localSecretPresent: false }]);
    render(<SyncConflictList onResolved={vi.fn()} workspaceId="workspace" />);
    expect(await screen.findByText("http://localhost")).toBeTruthy();
    expect(screen.getByText("https://cloud")).toBeTruthy();
    fireEvent.click(screen.getByText("Use this device"));
    await waitFor(() => expect(mocks.keepLocal).toHaveBeenCalled());
    await waitFor(() => expect((screen.getByText("Use cloud") as HTMLButtonElement).disabled).toBe(false));
    fireEvent.click(screen.getByText("Use cloud"));
    await waitFor(() => expect(mocks.useRemote).toHaveBeenCalled());
  });
});
