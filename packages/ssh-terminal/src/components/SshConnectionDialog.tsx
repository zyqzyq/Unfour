import { useState, type FormEvent, type ReactNode } from "react";
import { KeyRound, Lock, Plug, Save, ShieldOff } from "lucide-react";
import { listTemporaryRoutes, revealConnectionSecret, temporaryRouteOptions, type SshConnectionInput, type SshTestResult } from "@unfour/command-client";
import {
  Button,
  Dialog,
  DialogBody,
  DialogClose,
  DialogContent,
  DialogDescription,
  DialogFooter,
  DialogHeader,
  DialogTitle,
  DialogXClose,
  ErrorState,
  Input,
  SavedSecretInput,
  SegmentedControl,
  Select,
  useLiveOptions,
  useI18n,
} from "@unfour/ui";
import { formatTerminalError } from "../model/errors";
import { SshTestResultDialog } from "./SshTestResultDialog";

export function SshConnectionDialog({
  canTest,
  externalChange = false,
  onReload,
  error,
  form,
  onOpenChange,
  onSubmit,
  onTest,
  onUpdate,
  open,
  pending,
  testing,
  testResult = null,
  onCloseTestResult,
}: {
  canTest: boolean;
  externalChange?: boolean;
  onReload?: () => void;
  error?: unknown;
  form: SshConnectionInput;
  onOpenChange: (open: boolean) => void;
  onSubmit: (event: FormEvent) => void;
  onTest: () => void;
  onUpdate: (patch: Partial<SshConnectionInput>) => void;
  open: boolean;
  pending?: boolean;
  testing?: boolean;
  testResult?: SshTestResult | null;
  onCloseTestResult?: () => void;
}) {
  const { t } = useI18n();
  const sessions = useLiveOptions(form.workspaceId, listTemporaryRoutes, open);
  const temporary = temporaryRouteOptions(sessions?.items ?? [], form.temporaryHandle, (id) => t("network.temporarySession", { id }), t("network.temporaryUnavailable"));
  const busy = Boolean(pending || testing);

  return (
    <Dialog onOpenChange={(next) => { if (!busy) onOpenChange(next); }} open={open}>
      <DialogContent>
        <form className="flex min-h-0 flex-col" onSubmit={(event) => { if (externalChange || busy) event.preventDefault(); else onSubmit(event); }}>
          <fieldset aria-busy={busy} disabled={busy} className="contents">
          <DialogHeader>
            <div className="min-w-0">
              <DialogTitle>
                {form.id ? t("ssh.dialog.editTitle") : t("ssh.dialog.title")}
              </DialogTitle>
              <DialogDescription>{t("ssh.dialog.description")}</DialogDescription>
            </div>
            <DialogXClose />
          </DialogHeader>
          <DialogBody className="space-y-4">
            {externalChange && <div role="alert" className="text-[12px] text-[var(--u-color-text-muted)]">{t("common.externalChange")} <Button onClick={onReload} type="button" size="sm" variant="outline">{t("common.reloadLatest")}</Button></div>}
            <FieldGroup title={t("ssh.dialog.name")}>
              <Input
                autoFocus
                onChange={(event) => onUpdate({ name: event.target.value })}
                value={form.name}
              />
            </FieldGroup>
            <div className="grid grid-cols-[minmax(0,1fr)_84px] gap-2">
              <FieldGroup title={t("ssh.dialog.host")}>
                <Input onChange={(event) => onUpdate({ host: event.target.value })} value={form.host} />
              </FieldGroup>
              <FieldGroup title={t("ssh.dialog.port")}>
                <Input
                  onChange={(event) =>
                    onUpdate({ port: event.target.value ? Number(event.target.value) : null })
                  }
                  type="number"
                  value={form.port ?? ""}
                />
              </FieldGroup>
            </div>
            <FieldGroup title={t("ssh.dialog.username")}>
              <Input
                onChange={(event) => onUpdate({ username: event.target.value })}
                value={form.username}
              />
            </FieldGroup>
            <FieldGroup title={t("redis.route.label")}>
              <Select value={form.temporaryHandle ?? ""} options={[{ value: "", label: t("redis.route.direct") }, ...temporary]}
                onChange={(event) => onUpdate({ temporaryHandle: event.target.value || null })} />
              {form.temporaryHandle && <span className="text-xs text-[var(--u-color-text-muted)]">{t("network.temporaryHelp")}</span>}
            </FieldGroup>
            <fieldset className="min-w-0 space-y-1.5">
              <legend className="text-[12px] font-medium text-[var(--u-color-text-muted)]">{t("ssh.dialog.authentication")}</legend>
              <SegmentedControl<SshConnectionInput["authKind"]>
                onChange={(authKind) =>
                  onUpdate({
                    authKind,
                    keyPath: authKind === "private-key" ? form.keyPath : null,
                    secret: null,
                    credentialRef: null,
                  })
                }
                options={[
                  {
                    icon: <Lock size={14} />,
                    label: t("ssh.dialog.authPassword"),
                    value: "password",
                  },
                  {
                    icon: <KeyRound size={14} />,
                    label: t("ssh.dialog.authPrivateKey"),
                    value: "private-key",
                  },
                  {
                    icon: <ShieldOff size={14} />,
                    label: t("ssh.dialog.authNone"),
                    value: "none",
                  },
                ]}
                value={form.authKind}
              />
            </fieldset>
            {form.authKind === "password" && (
              <FieldGroup title={t("ssh.dialog.authPassword")}>
                {open && <ConnectionSecretEditor key={`${form.workspaceId}:${form.id ?? "new"}:${form.expectedSnapshot?.credentialRef ?? ""}:${form.authKind}`} form={form} onUpdate={onUpdate} disabled={busy} />}
                {Boolean(form.id) && (
                  <span className="text-[11.5px] text-[var(--u-color-text-muted)]">
                    {t("ssh.dialog.passwordEditHint")}
                  </span>
                )}
              </FieldGroup>
            )}
            {form.authKind === "private-key" && (
              <>
                <FieldGroup title={t("ssh.dialog.keyPath")}>
                  <Input
                    onChange={(event) => onUpdate({ keyPath: event.target.value })}
                    placeholder={t("ssh.dialog.keyPathPlaceholder")}
                    value={form.keyPath ?? ""}
                  />
                </FieldGroup>
                <FieldGroup title={t("ssh.dialog.passphrase")}>
                  {open && <ConnectionSecretEditor key={`${form.workspaceId}:${form.id ?? "new"}:${form.expectedSnapshot?.credentialRef ?? ""}:${form.authKind}`} form={form} onUpdate={onUpdate} disabled={busy} />}
                  {Boolean(form.id) && (
                    <span className="text-[11.5px] text-[var(--u-color-text-muted)]">
                      {t("ssh.dialog.passwordEditHint")}
                    </span>
                  )}
                </FieldGroup>
              </>
            )}
            {form.authKind === "none" && (
              <p className="text-[11.5px] leading-relaxed text-[var(--u-color-text-muted)]">
                {t("ssh.dialog.authNoneHint")}
              </p>
            )}
            {Boolean(error) && (
              <ErrorState className="min-h-0 justify-start py-2 text-left">
                {formatTerminalError(error, t)}
              </ErrorState>
            )}
          </DialogBody>
          <DialogFooter>
            <SshTestResultDialog onClose={onCloseTestResult} result={testResult}>
              <Button
                className="mr-auto"
                disabled={!canTest || busy}
                onClick={onTest}
                type="button"
                variant="outline"
              >
                <Plug size={14} />
                {testing ? t("ssh.dialog.testing") : t("ssh.dialog.test")}
              </Button>
            </SshTestResultDialog>
            <DialogClose asChild>
              <Button type="button" variant="outline">
                {t("ssh.dialog.cancel")}
              </Button>
            </DialogClose>
            <Button disabled={busy || externalChange} type="submit">
              <Save size={14} />
              {t("ssh.dialog.save")}
            </Button>
          </DialogFooter>
          </fieldset>
        </form>
      </DialogContent>
    </Dialog>
  );
}

function FieldGroup({ children, title }: { children: ReactNode; title: string }) {
  return (
    <label className="block space-y-1.5">
      <span className="text-[12px] font-medium text-[var(--u-color-text-muted)]">
        {title}
      </span>
      {children}
    </label>
  );
}

function ConnectionSecretEditor({ form, onUpdate, disabled }: { form: SshConnectionInput; onUpdate: (patch: Partial<SshConnectionInput>) => void; disabled?: boolean }) {
  const [savedReference] = useState(form.credentialRef);
  return <SavedSecretInput saved={Boolean(savedReference)} cleared={Boolean(savedReference) && !form.credentialRef} disabled={disabled}
    value={form.secret ?? ""} onChange={(secret) => onUpdate({ secret, credentialRef: savedReference })}
    onKeep={() => onUpdate({ secret: null, credentialRef: savedReference })}
    onClear={() => onUpdate({ secret: null, credentialRef: null })}
    onReveal={() => revealConnectionSecret({ workspaceId: form.workspaceId, connectionId: form.id!, connectionType: "ssh", credentialRef: savedReference! })} />;
}
