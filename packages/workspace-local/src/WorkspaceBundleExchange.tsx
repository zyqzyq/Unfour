import { useState } from "react";
import { useQueryClient } from "@tanstack/react-query";
import { exportWorkspaceBundle, importWorkspaceBundle, pickWorkspaceBundle, previewWorkspaceBundle, type Workspace, type WorkspaceBundleFile, type WorkspaceBundleOptions, type WorkspaceMcpPolicy } from "@unfour/command-client";
import { Badge, Button, Dialog, DialogBody, DialogContent, DialogDescription, DialogFooter, DialogHeader, DialogTitle, Input, Select, useFeedback, useFeedbackErrorHandler, useI18n } from "@unfour/ui";
import { WorkspaceMcpPolicyField } from "./WorkspaceMcpPolicyField";

export function useWorkspaceBundleExchange(onImported: (id: string) => void) {
  const { t } = useI18n();
  const feedback = useFeedback();
  const handleError = useFeedbackErrorHandler();
  const queryClient = useQueryClient();
  const [pending, setPending] = useState<WorkspaceBundleFile | null>(null);
  const [name, setName] = useState("");
  const [mcpPolicy, setMcpPolicy] = useState<WorkspaceMcpPolicy>("disabled");
  const [busy, setBusy] = useState(false);
  const [exportTarget, setExportTarget] = useState<Pick<Workspace, "id" | "name" | "environmentType"> | null>(null);
  const [mode, setMode] = useState("share");
  const [password, setPassword] = useState("");
  const [confirmation, setConfirmation] = useState("");
  const [keepPaths, setKeepPaths] = useState(false);
  const [includeSecrets, setIncludeSecrets] = useState(true);
  const [from, setFrom] = useState("");
  const [to, setTo] = useState("");
  const [overrides, setOverrides] = useState<NonNullable<WorkspaceBundleOptions["pathOverrides"]>>([]);
  const [reviewed, setReviewed] = useState(true);
  const [errorKey, setErrorKey] = useState<string | null>(null);
  function reset() {
    setMcpPolicy("disabled");
    setPassword(""); setConfirmation(""); setFrom(""); setTo(""); setOverrides([]); setReviewed(true); setErrorKey(null);
  }
  function close() { setPending(null); setExportTarget(null); reset(); }
  const importOptions = (): WorkspaceBundleOptions => ({
    ...(pending?.encrypted ? { password } : {}),
    ...(from && to ? { pathMappings: [{ from, to }] } : {}),
    ...(overrides.length ? { pathOverrides: overrides } : {}),
  });
  async function run(action: () => Promise<void>) {
    setBusy(true); setErrorKey(null);
    try { await action(); }
    catch (error) {
      const message = typeof error === "string" ? error : error instanceof Error ? error.message : JSON.stringify(error);
      const code = ["UNLOCK_FAILED", "PASSWORD_LENGTH", "CREDENTIAL_UNAVAILABLE", "CLEANUP_PENDING", "TOO_LARGE"].find((value) => message?.includes(`WORKSPACE_BUNDLE_${value}`));
      if (code) setErrorKey(`workspaceBundle.errors.${code}`);
      else handleError(error, { key: "workspaceBundle.failed" });
    } finally { setBusy(false); }
  }
  function pick() {
    return run(async () => {
      const file = await pickWorkspaceBundle();
      if (file) { reset(); setPending(file); setName(file.preview?.name ?? ""); }
    });
  }
  function review(checkPaths = false) {
    return run(async () => {
      if (!pending) return;
      const preview = await previewWorkspaceBundle(pending.content, { ...importOptions(), checkPaths });
      setPending({ ...pending, preview }); if (!name) setName(preview.name); setReviewed(true);
    });
  }
  function confirmImport() {
    return run(async () => {
      if (!pending?.preview || !reviewed || !name.trim()) return;
      const workspace = await importWorkspaceBundle(pending.content, name.trim(), { ...importOptions(), mcpPolicy });
      await queryClient.invalidateQueries({ queryKey: ["workspaces"] });
      close(); onImported(workspace.id); feedback.success(t("exchange.imported", { name: workspace.name }));
    });
  }
  function confirmExport() {
    return run(async () => {
      if (!exportTarget) return;
      const result = await exportWorkspaceBundle(exportTarget.id, { keepLocalPaths: keepPaths, includeSecrets: mode === "backup" && includeSecrets, ...(mode === "backup" ? { password } : {}) });
      if (result.saved) { close(); feedback.success(t("exchange.exported")); }
    });
  }
  const preview = pending?.preview;
  const paths = [...(preview?.paths ?? []), ...(preview?.reconfigure ?? []).filter((issue) => issue.code === "localPath" && issue.field).map((issue) => ({ entityId: issue.entityId, field: issue.field!, path: "" }))];
  const needsReview = pending && (!preview || !reviewed);
  const exportValid = mode !== "backup" || (Array.from(password).length >= 12 && password === confirmation);
  const mappingValid = Boolean(from) === Boolean(to) && overrides.every((path) => path.path.trim().length > 0);
  const dialog = <Dialog open={pending !== null || exportTarget !== null} onOpenChange={(open) => { if (!open && !busy) close(); }}>
    <DialogContent onEscapeKeyDown={(event) => { if (busy) event.preventDefault(); }} onPointerDownOutside={(event) => { if (busy) event.preventDefault(); }}>
      <DialogHeader><DialogTitle>{t(pending ? "workspaceBundle.preview" : "workspaceBundle.exportTitle")}</DialogTitle></DialogHeader>
      <DialogBody>
        {exportTarget && <div className="mb-3 flex min-w-0 items-start gap-2 border-b border-[var(--u-color-border)] pb-3 text-sm">
          <span className="shrink-0 text-[var(--u-color-text-muted)]">{t("workspaceBundle.target")}</span><span className="min-w-0 break-words font-medium">{exportTarget.name}</span>
          <Badge tone={exportTarget.environmentType === "prod" ? "red" : exportTarget.environmentType === "test" ? "amber" : "green"}>{exportTarget.environmentType.toUpperCase()}</Badge>
        </div>}
        <DialogDescription className="mb-3">{t(pending ? "workspaceBundle.newOnly" : "workspaceBundle.exportDescription")}</DialogDescription>
        {exportTarget && <div className="mb-3 space-y-3 text-xs">
          <label className="block">{t("workspaceBundle.mode")}<Select aria-label={t("workspaceBundle.mode")} className="mt-1" value={mode} disabled={busy} options={[{ value: "share", label: t("workspaceBundle.share") }, { value: "backup", label: t("workspaceBundle.backup") }]} onChange={(event) => { setMode(event.target.value); setKeepPaths(event.target.value === "backup"); setPassword(""); setConfirmation(""); }} /></label>
          <label className="flex items-center gap-2"><input type="checkbox" checked={keepPaths} disabled={busy} onChange={(event) => setKeepPaths(event.target.checked)} />{t("workspaceBundle.keepPaths")}</label>
          {mode === "backup" && <label className="flex items-center gap-2"><input type="checkbox" checked={includeSecrets} disabled={busy} onChange={(event) => setIncludeSecrets(event.target.checked)} />{t("workspaceBundle.includeSecrets")}</label>}
        </div>}
        {(pending?.encrypted || (exportTarget && mode === "backup")) && <div className="mb-3 space-y-2 text-xs">
          <label className="block">{t("workspaceBundle.password")}<Input type="password" autoComplete={exportTarget ? "new-password" : "off"} maxLength={256} disabled={busy} value={password} onChange={(event) => { setPassword(event.target.value); if (pending) setReviewed(false); }} /></label>
          {exportTarget && <label className="block">{t("workspaceBundle.confirmPassword")}<Input type="password" autoComplete="new-password" maxLength={256} disabled={busy} value={confirmation} onChange={(event) => setConfirmation(event.target.value)} /></label>}
          <p className="text-[var(--u-color-text-muted)]">{t(exportTarget ? "workspaceBundle.passwordHelp" : "workspaceBundle.unlockHelp")}</p>
        </div>}
        <p className="mb-3 text-xs text-[var(--u-color-text-muted)]">{t("workspaceBundle.excluded")}</p>
        {exportTarget && <p className="mb-3 text-xs text-[var(--u-color-text-muted)]">{t(mode === "backup" ? "workspaceBundle.backupHelp" : "workspaceBundle.exportReview")}</p>}
        {preview && <>
          <label className="mb-3 block text-xs">{t("workspaceBundle.name")}<Input className="mt-1" maxLength={80} value={name} disabled={busy} onChange={(event) => setName(event.target.value)} /></label>
          <div className="mb-3 space-y-1.5">
            <WorkspaceMcpPolicyField disabled={busy} value={mcpPolicy} environmentType={preview.environmentType} environmentSource="import" onChange={setMcpPolicy} />
            <p className="text-xs text-[var(--u-color-text-muted)]">{t("workspaceBundle.mcpPolicyHint")}</p>
          </div>
          <dl className="grid grid-cols-2 gap-x-4 gap-y-1 text-xs">{Object.entries(preview.counts).map(([key, count]) => <div key={key} className="flex justify-between gap-2"><dt>{t(`workspaceBundle.counts.${key}`)}</dt><dd>{count}</dd></div>)}</dl>
          <p className="my-3 text-xs text-[var(--u-color-text-muted)]">{t("workspaceBundle.review")}</p>
          {["missing", "unchecked", "ready"].map((status) => {
            const issues = preview.reconfigure.filter((issue) => (issue.status ?? "missing") === status);
            return issues.length > 0 && <div key={status} className="mt-3 border-t border-[var(--u-color-border)] pt-3"><p className="mb-2 text-xs font-medium">{t(`workspaceBundle.status.${status}`, { count: issues.length })}</p>
              <ul className="max-h-32 space-y-1 overflow-y-auto text-xs text-[var(--u-color-text-muted)]">{issues.map((issue, index) => <li key={`${issue.entityId}-${issue.field}-${index}`}><span>{issue.name}</span>{issue.field && ` · ${t(`workspaceBundle.fields.${issue.field}`)}`} — {t(`workspaceBundle.issues.${issue.code}`)}</li>)}</ul></div>;
          })}
          {paths.length > 0 && <details className="mt-3 text-xs"><summary className="cursor-pointer">{t("workspaceBundle.paths")}</summary><div className="mt-2 space-y-2">
            <label className="block">{t("workspaceBundle.mapFrom")}<Input disabled={busy} value={from} onChange={(event) => { setFrom(event.target.value); setReviewed(false); }} /></label>
            <label className="block">{t("workspaceBundle.mapTo")}<Input disabled={busy} value={to} onChange={(event) => { setTo(event.target.value); setReviewed(false); }} /></label>
            {paths.map((path) => <label key={`${path.entityId}-${path.field}`} className="block">{preview.reconfigure.find((issue) => issue.entityId === path.entityId)?.name} · {t(`workspaceBundle.fields.${path.field}`)}<Input disabled={busy} value={overrides.find((p) => p.entityId === path.entityId && p.field === path.field)?.path ?? path.path} onChange={(event) => { setOverrides((current) => [...current.filter((p) => p.entityId !== path.entityId || p.field !== path.field), { ...path, path: event.target.value }]); setReviewed(false); }} /></label>)}
            <Button variant="secondary" disabled={busy || !mappingValid} onClick={() => void review(true)}>{t("workspaceBundle.checkPaths")}</Button>
          </div></details>}
        </>}
        {errorKey && <p role="alert" className="mt-3 text-xs text-[var(--u-color-danger)]">{t(errorKey)}</p>}
      </DialogBody>
      <DialogFooter><Button variant="secondary" disabled={busy} onClick={close}>{t("common.confirm.cancel")}</Button>
        <Button disabled={busy || (pending ? !mappingValid || (needsReview ? Boolean(pending.encrypted && !password) : !name.trim()) : !exportValid)} onClick={() => void (pending ? needsReview ? review() : confirmImport() : confirmExport())}>{t(busy ? "workspaceBundle.working" : pending ? needsReview ? "workspaceBundle.unlockReview" : "workspaceBundle.create" : "workspaceBundle.save")}</Button>
      </DialogFooter>
    </DialogContent>
  </Dialog>;
  return { pick, exportWorkspace: (workspace: Pick<Workspace, "id" | "name" | "environmentType">) => { reset(); setMode("share"); setKeepPaths(false); setIncludeSecrets(true); setExportTarget({ ...workspace }); }, busy, dialog };
}
