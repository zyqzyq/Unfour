import type { WorkspaceEnvironmentType, WorkspaceMcpPolicy } from "@unfour/command-client";
import { Select, useI18n } from "@unfour/ui";
import { resolveWorkspaceMcpPolicy } from "@unfour/workspace-core";

const policies: WorkspaceMcpPolicy[] = ["auto", "disabled", "read_only", "guarded", "full_access"];

export function WorkspaceMcpPolicyField({ value, onChange, disabled, environmentType }: {
  value: WorkspaceMcpPolicy;
  onChange: (policy: WorkspaceMcpPolicy) => void;
  disabled?: boolean;
  environmentType?: WorkspaceEnvironmentType;
}) {
  const { t } = useI18n();
  return <div className="space-y-1.5 text-xs">
    <label className="block space-y-1.5">
      <span className="font-medium">{t("app.workspace.mcp.label")}</span>
      <Select
        aria-label={t("app.workspace.mcp.label")}
        disabled={disabled}
        value={value}
        onChange={(event) => onChange(event.target.value as WorkspaceMcpPolicy)}
        options={policies.map((policy) => ({
          value: policy,
          label: `${t(`app.workspace.mcp.options.${policy}`)} — ${t(`app.workspace.mcp.descriptions.${policy}`)}`,
        }))}
      />
    </label>
    <p className="text-[var(--u-color-text-muted)]">{t(`app.workspace.mcp.descriptions.${value}`)}</p>
    {value === "auto" && <p className="text-[var(--u-color-text-muted)]">
      {environmentType
        ? t("app.workspace.mcp.autoEffective", { policy: t(`app.workspace.mcp.options.${resolveWorkspaceMcpPolicy({ environmentType, mcpPolicy: value })}`) })
        : t("app.workspace.mcp.autoRules")}
    </p>}
  </div>;
}
