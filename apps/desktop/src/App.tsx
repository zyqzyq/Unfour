import {
  DesktopApp,
  type DesktopAppExtensionContext,
  type DesktopAppExtensions,
  type DesktopAppWorkspaceActionsProvider,
} from "@unfour/app-shell";
import { useI18n } from "@unfour/ui";
import {
  AccountIndicator,
  AccountOverlays,
  AccountProvider,
} from "./features/account";
import {
  CloudSyncOverlays,
  CloudSyncProvider,
  CloudSyncStatus,
  CloudSyncWorkspaceDecoration,
  useCloudSync,
} from "./features/cloud-sync";
import { createCloudSyncWorkspaceActions } from "./features/cloud-sync/workspaceMenuActions";
import {
  UpdateIndicator,
  UpdateOverlays,
  UpdateProvider,
} from "./features/update";
import {
  TelemetryNotice,
  TelemetryProvider,
} from "./features/telemetry";
import { desktopSettingsSections } from "./desktopSettingsSections";

function DesktopTitleBarEnd(context: DesktopAppExtensionContext) {
  return <><CloudSyncStatus {...context} /><AccountIndicator /><UpdateIndicator /></>;
}

function DesktopOverlays(context: DesktopAppExtensionContext) {
  return <><TelemetryNotice /><AccountOverlays /><UpdateOverlays /><CloudSyncOverlays {...context} /></>;
}

function ExtendedDesktopApp() {
  const { t } = useI18n();
  const cloudSync = useCloudSync();
  const workspaceMenuActions: DesktopAppWorkspaceActionsProvider = (_context, workspace) =>
    createCloudSyncWorkspaceActions(cloudSync, t, workspace);
  const extensions: DesktopAppExtensions = {
    titleBarEnd: DesktopTitleBarEnd,
    settingsSections: desktopSettingsSections,
    workspaceDecoration: CloudSyncWorkspaceDecoration,
    workspaceMenuActions,
    workspaceMenuFooterActions: [{
      id: "cloud-sync.open-cloud-workspace",
      label: t("cloudSync.openCloudWorkspace"),
      disabled: !cloudSync.available,
      run: cloudSync.openCloudWorkspaceDialog,
    }],
    overlays: DesktopOverlays,
  };

  return <DesktopApp extensions={extensions} />;
}

function App() {
  return (
    <TelemetryProvider>
      <UpdateProvider>
        <AccountProvider>
          <CloudSyncProvider>
            <ExtendedDesktopApp />
          </CloudSyncProvider>
        </AccountProvider>
      </UpdateProvider>
    </TelemetryProvider>
  );
}

export default App;
