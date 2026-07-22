import { StatusBar, useI18n } from "@unfour/ui";

export function WorkspaceEnvironmentsStatusBar({
  workspaceName,
}: {
  workspaceName: string;
}) {
  const { t } = useI18n();
  return (
    <StatusBar>
      <div className="flex min-w-0 items-center gap-3">
        <span className="shrink-0">{t("app.status.ready")}</span>
        <span className="truncate">{workspaceName}</span>
        <span className="shrink-0">{t("variables.managerStatus")}</span>
      </div>
    </StatusBar>
  );
}
