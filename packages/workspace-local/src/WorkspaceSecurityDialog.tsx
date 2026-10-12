import { useState } from "react";
import { useMutation, useQueryClient } from "@tanstack/react-query";
import { updateWorkspaceEnvironment, updateWorkspaceMcpPolicy, type Workspace, type WorkspaceEnvironmentType, type WorkspaceMcpPolicy, type WorkspaceState } from "@unfour/command-client";
import { Button, Dialog, DialogBody, DialogContent, DialogDescription, DialogFooter, DialogHeader, DialogTitle, Select, useI18n } from "@unfour/ui";
import { resolveWorkspaceMcpPolicy } from "@unfour/workspace-core";
import { WorkspaceMcpPolicyField } from "./WorkspaceMcpPolicyField";
import { WorkspaceMcpRiskNotice } from "./WorkspaceMcpRiskNotice";

/** Mounted for the selected workspace while its security settings are open. */
export function WorkspaceSecurityDialog({ workspace, onClose }: {
  workspace: Workspace;
  onClose: () => void;
}) {
  const { t } = useI18n();
  const queryClient = useQueryClient();
  const [saved, setSaved] = useState(workspace);
  const [environmentDraft, setEnvironmentDraft] = useState(workspace.environmentType);
  const [policyDraft, setPolicyDraft] = useState(workspace.mcpPolicy);

  function didSave(updated: Workspace) {
    setSaved(updated);
    queryClient.setQueryData<WorkspaceState>(["workspaces"], (current) => current && ({
      ...current,
      workspaces: current.workspaces.map((item) => item.id === updated.id ? updated : item),
    }));
    void queryClient.invalidateQueries({ queryKey: ["workspaces"] });
  }
  const environmentMutation = useMutation({
    mutationFn: (value: WorkspaceEnvironmentType) => updateWorkspaceEnvironment(workspace.id, value),
    onSuccess: didSave,
  });
  const policyMutation = useMutation({
    mutationFn: (value: WorkspaceMcpPolicy) => updateWorkspaceMcpPolicy(workspace.id, value),
    onSuccess: didSave,
  });
  const busy = environmentMutation.isPending || policyMutation.isPending;
  return <Dialog open onOpenChange={(open) => { if (!open && !busy) onClose(); }}>
    <DialogContent onEscapeKeyDown={(event) => { if (busy) event.preventDefault(); }} onPointerDownOutside={(event) => { if (busy) event.preventDefault(); }}>
      <DialogHeader><DialogTitle>{t("app.workspace.dialog.securityTitle")}</DialogTitle></DialogHeader>
      <DialogBody className="space-y-4">
        <DialogDescription>{t("app.workspace.dialog.securityDescription")}</DialogDescription>
        <form className="space-y-2" onSubmit={(event) => { event.preventDefault(); if (!busy && environmentDraft !== saved.environmentType) environmentMutation.mutate(environmentDraft); }}>
          <label className="block space-y-1.5 text-xs">
            <span className="font-medium">{t("app.workspace.environment.label")}</span>
            <Select aria-label={t("app.workspace.environment.label")} autoFocus disabled={busy} value={environmentDraft} onChange={(event) => { setEnvironmentDraft(event.target.value as WorkspaceEnvironmentType); environmentMutation.reset(); }} options={[
              { value: "dev", label: `DEV · ${t("app.workspace.environment.development")}` },
              { value: "test", label: `TEST · ${t("app.workspace.environment.test")}` },
              { value: "prod", label: `PROD · ${t("app.workspace.environment.production")}` },
            ]} />
          </label>
          <p className="text-xs text-[var(--u-color-text-muted)]">{t("app.workspace.environment.hint")}</p>
          <p className="text-xs text-[var(--u-color-text-muted)]">{t("app.workspace.environment.current", { environment: saved.environmentType.toUpperCase() })}</p>
          <WorkspaceMcpRiskNotice environmentType={environmentDraft} policy={saved.mcpPolicy} context={t("app.workspace.environment.saveRiskContext")} />
          {environmentMutation.isError && <p role="alert" className="text-xs text-[var(--u-color-danger)]">{t("feedback.workspace.environmentFailed")}</p>}
          <div className="flex justify-end"><Button disabled={busy || environmentDraft === saved.environmentType} type="submit">{t("app.workspace.dialog.saveEnvironment")}</Button></div>
        </form>
        <form className="space-y-2 border-t border-[var(--u-color-border)] pt-3" onSubmit={(event) => { event.preventDefault(); if (!busy && policyDraft !== saved.mcpPolicy) policyMutation.mutate(policyDraft); }}>
          <WorkspaceMcpPolicyField disabled={busy} value={policyDraft} environmentType={saved.environmentType} onChange={(value) => { setPolicyDraft(value); policyMutation.reset(); }} />
          <p className="text-xs text-[var(--u-color-text-muted)]">{t("app.workspace.mcp.current", {
            policy: saved.mcpPolicy === "auto"
              ? `${t("app.workspace.mcp.options.auto")} → ${t(`app.workspace.mcp.options.${resolveWorkspaceMcpPolicy(saved)}`)}`
              : t(`app.workspace.mcp.options.${saved.mcpPolicy}`),
          })}</p>
          <WorkspaceMcpRiskNotice environmentType={saved.environmentType} policy={policyDraft} context={t("app.workspace.mcp.saveRiskContext")} />
          {policyMutation.isError && <p role="alert" className="text-xs text-[var(--u-color-danger)]">{t("feedback.workspace.mcpPolicyFailed")}</p>}
          <div className="flex justify-end"><Button disabled={busy || policyDraft === saved.mcpPolicy} type="submit">{t("app.workspace.dialog.saveMcpPolicy")}</Button></div>
        </form>
      </DialogBody>
      <DialogFooter><Button variant="secondary" disabled={busy} onClick={onClose}>{t("app.workspace.dialog.close")}</Button></DialogFooter>
    </DialogContent>
  </Dialog>;
}
