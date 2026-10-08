import { useState } from "react";
import { useQueryClient } from "@tanstack/react-query";
import { exportWorkspaceBundle, importWorkspaceBundle, pickWorkspaceBundle, type Workspace, type WorkspaceBundleFile } from "@unfour/command-client";
import { Badge, Button, Dialog, DialogBody, DialogContent, DialogDescription, DialogFooter, DialogHeader, DialogTitle, Input, useFeedback, useFeedbackErrorHandler, useI18n } from "@unfour/ui";

/** Local workspace lifecycle belongs here; the shell only mounts menu actions and the dialog. */
export function useWorkspaceBundleExchange(onImported: (id: string) => void) {
  const { t } = useI18n();
  const feedback = useFeedback();
  const handleError = useFeedbackErrorHandler();
  const queryClient = useQueryClient();
  const [pending, setPending] = useState<WorkspaceBundleFile | null>(null);
  const [name, setName] = useState("");
  const [busy, setBusy] = useState(false);
  const [exportTarget, setExportTarget] = useState<Pick<Workspace, "id" | "name" | "environmentType"> | null>(null);
  async function run(action: () => Promise<void>) {
    setBusy(true);
    try { await action(); }
    catch (error) { handleError(error, { key: "workspaceBundle.failed" }); }
    finally { setBusy(false); }
  }
  function pick() {
    return run(async () => {
      const file = await pickWorkspaceBundle();
      if (file) { setPending(file); setName(file.preview.name); }
    });
  }
  function confirmImport() {
    return run(async () => {
      if (!pending || !name.trim()) return;
      const workspace = await importWorkspaceBundle(pending.content, name.trim());
      await queryClient.invalidateQueries({ queryKey: ["workspaces"] });
      setPending(null);
      onImported(workspace.id);
      feedback.success(t("exchange.imported", { name: workspace.name }));
    });
  }
  function confirmExport() {
    return run(async () => {
      if (!exportTarget) return;
      const result = await exportWorkspaceBundle(exportTarget.id);
      setExportTarget(null);
      if (result.saved) feedback.success(t("exchange.exported"));
    });
  }
  const dialog = (
    <Dialog open={pending !== null || exportTarget !== null} onOpenChange={(open) => { if (!open && !busy) { setPending(null); setExportTarget(null); } }}>
      <DialogContent onEscapeKeyDown={(event) => { if (busy) event.preventDefault(); }} onPointerDownOutside={(event) => { if (busy) event.preventDefault(); }}>
        <DialogHeader>
          <DialogTitle>{t(pending ? "workspaceBundle.preview" : "workspaceBundle.exportTitle")}</DialogTitle>
        </DialogHeader>
        <DialogBody>
          {pending && <label className="mb-3 block text-xs">{t("workspaceBundle.name")}
            <Input autoFocus className="mt-1" maxLength={80} value={name} disabled={busy} onChange={(event) => setName(event.target.value)} />
          </label>}
          {exportTarget && <div className="mb-3 flex min-w-0 items-start gap-2 border-b border-[var(--u-color-border)] pb-3 text-sm">
            <span className="shrink-0 text-[var(--u-color-text-muted)]">{t("workspaceBundle.target")}</span>
            <span className="min-w-0 break-words font-medium">{exportTarget.name}</span>
            <Badge tone={exportTarget.environmentType === "prod" ? "red" : exportTarget.environmentType === "test" ? "amber" : "green"}>
              {exportTarget.environmentType.toUpperCase()}
            </Badge>
          </div>}
          <DialogDescription className="mb-3">{t(pending ? "workspaceBundle.newOnly" : "workspaceBundle.exportDescription")}</DialogDescription>
          <dl className="grid grid-cols-[auto_minmax(0,1fr)] gap-x-3 gap-y-2 text-xs">
            <dt>{t("workspaceBundle.includedLabel")}</dt><dd>{t("workspaceBundle.included")}</dd>
            <dt className="text-[var(--u-color-text-muted)]">{t("workspaceBundle.excludedLabel")}</dt>
            <dd className="text-[var(--u-color-text-muted)]">{t("workspaceBundle.excluded")}</dd>
          </dl>
          <p className="my-3 text-xs text-[var(--u-color-text-muted)]">{t(pending ? "workspaceBundle.review" : "workspaceBundle.exportReview")}</p>
          {pending && <>
            <dl className="grid grid-cols-2 gap-x-4 gap-y-1 text-xs">
              {Object.entries(pending.preview.counts).map(([key, count]) => <div key={key} className="flex justify-between gap-2"><dt>{t(`workspaceBundle.counts.${key}`)}</dt><dd>{count}</dd></div>)}
            </dl>
            {pending.preview.reconfigure.length > 0 && <div className="mt-3 border-t border-[var(--u-color-border)] pt-3">
              <p className="mb-2 font-medium">{t("workspaceBundle.reconfigure", { count: pending.preview.reconfigure.length })}</p>
              <ul className="max-h-40 space-y-1 overflow-y-auto text-xs text-[var(--u-color-text-muted)]">
                {pending.preview.reconfigure.map((issue, index) => <li key={`${issue.entityId}-${index}`}><span title={issue.entityId}>{issue.name}</span> — {t(`workspaceBundle.issues.${issue.code}`)}</li>)}
              </ul>
            </div>}
          </>}
        </DialogBody>
        <DialogFooter>
          <Button variant="secondary" disabled={busy} onClick={() => { setPending(null); setExportTarget(null); }}>{t("common.confirm.cancel")}</Button>
          <Button disabled={busy || (pending !== null && !name.trim())} onClick={() => void (pending ? confirmImport() : confirmExport())}>{t(busy ? "workspaceBundle.working" : pending ? "workspaceBundle.create" : "workspaceBundle.save")}</Button>
        </DialogFooter>
      </DialogContent>
    </Dialog>
  );
  return { pick, exportWorkspace: (workspace: Pick<Workspace, "id" | "name" | "environmentType">) => setExportTarget({ id: workspace.id, name: workspace.name, environmentType: workspace.environmentType }), busy, dialog };
}
