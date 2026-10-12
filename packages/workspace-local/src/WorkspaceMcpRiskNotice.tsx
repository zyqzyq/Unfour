import type { WorkspaceEnvironmentType, WorkspaceMcpPolicy } from "@unfour/command-client";
import { useI18n } from "@unfour/ui";

/** Explain the PROD combination that the associated action will actually apply. */
export function WorkspaceMcpRiskNotice({ environmentType, policy, context }: {
  environmentType?: WorkspaceEnvironmentType;
  policy: WorkspaceMcpPolicy;
  context?: string;
}) {
  const { t } = useI18n();
  if (environmentType !== "prod" || (policy !== "auto" && policy !== "guarded" && policy !== "full_access")) return null;
  return <p role="status" className="rounded border border-[var(--u-badge-danger-ring)] bg-[var(--u-badge-danger-bg)] px-3 py-2 text-xs text-[var(--u-badge-danger-text)]">
    {context && <span className="block">{context}</span>}
    {t(policy === "auto" ? "app.workspace.environment.prodWarning" : "app.workspace.environment.prodOverrideWarning", { policy: t(`app.workspace.mcp.options.${policy}`) })}
  </p>;
}
