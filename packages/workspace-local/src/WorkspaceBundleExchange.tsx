import { useState } from "react";
import { useQueryClient } from "@tanstack/react-query";
import { exportWorkspaceBundle, importWorkspaceBundle, pickWorkspaceBundle, type WorkspaceBundleFile } from "@unfour/command-client";
import { Button, Dialog, DialogBody, DialogContent, DialogDescription, DialogFooter, DialogHeader, DialogTitle, Input, useFeedback, useFeedbackErrorHandler, useI18n } from "@unfour/ui";

/** Local workspace lifecycle belongs here; the shell only mounts menu actions and the dialog. */
export function useWorkspaceBundleExchange(onImported: (id: string) => void) {
  const { t } = useI18n();
  const feedback = useFeedback();
  const handleError = useFeedbackErrorHandler();
  const queryClient = useQueryClient();
  const [pending, setPending] = useState<WorkspaceBundleFile | null>(null);
  const [name, setName] = useState("");
  const [busy, setBusy] = useState(false);
  const [exportId, setExportId] = useState<string | null>(null);
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
      if (!exportId) return;
      const result = await exportWorkspaceBundle(exportId);
      setExportId(null);
      if (result.saved) feedback.success(t("exchange.exported"));
    });
  }
  const dialog = (
    <Dialog open={pending !== null || exportId !== null} onOpenChange={(open) => { if (!open && !busy) { setPending(null); setExportId(null); } }}>
      <DialogContent onEscapeKeyDown={(event) => { if (busy) event.preventDefault(); }} onPointerDownOutside={(event) => { if (busy) event.preventDefault(); }}>
        <DialogHeader><DialogTitle>{t(pending ? "workspaceBundle.preview" : "workspaceBundle.export")}</DialogTitle></DialogHeader>
        <DialogBody>
          <DialogDescription>{t("workspaceBundle.scope")}</DialogDescription>
          <p className="my-3 text-xs text-[var(--u-color-text-muted)]">{t("workspaceBundle.review")}</p>
          {pending && <>
            <label className="mb-3 block text-xs">{t("workspaceBundle.name")}
              <Input autoFocus className="mt-1" maxLength={80} value={name} disabled={busy} onChange={(event) => setName(event.target.value)} />
            </label>
            <p className="mb-3 text-xs text-[var(--u-color-text-muted)]">{t("workspaceBundle.newOnly")}</p>
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
          <Button variant="secondary" disabled={busy} onClick={() => { setPending(null); setExportId(null); }}>{t("common.confirm.cancel")}</Button>
          <Button disabled={busy || (pending !== null && !name.trim())} onClick={() => void (pending ? confirmImport() : confirmExport())}>{t(busy ? "workspaceBundle.working" : pending ? "workspaceBundle.create" : "workspaceBundle.export")}</Button>
        </DialogFooter>
      </DialogContent>
    </Dialog>
  );
  return { pick, exportWorkspace: setExportId, busy, dialog };
}
