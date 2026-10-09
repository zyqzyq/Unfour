import { useState, type FormEvent, type ReactNode } from "react";
import { KeyRound, Lock, Plug, Save, ShieldOff } from "lucide-react";
import { revealConnectionSecret, type SshConnectionInput } from "@unfour/command-client";
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
  useI18n,
} from "@unfour/ui";
import { formatTerminalError } from "../model/errors";

export function SshConnectionDialog({
  canTest,
  error,
  form,
  onOpenChange,
  onSubmit,
  onTest,
  onUpdate,
  open,
  pending,
  testing,
  children,
}: {
  canTest: boolean;
  error?: unknown;
  form: SshConnectionInput;
  onOpenChange: (open: boolean) => void;
  onSubmit: (event: FormEvent) => void;
  onTest: () => void;
  onUpdate: (patch: Partial<SshConnectionInput>) => void;
  open: boolean;
  pending?: boolean;
  testing?: boolean;
  children?: ReactNode;
}) {
  const { t } = useI18n();

  return (
    <Dialog onOpenChange={onOpenChange} open={open}>
      <DialogContent>
        <form className="flex min-h-0 flex-col" onSubmit={onSubmit}>
          <DialogHeader>
            <div className="min-w-0">
              <DialogTitle>
                {form.id ? t("ssh.dialog.editTitle") : t("ssh.dialog.title")}
              </DialogTitle>
              <DialogDescription>{t("ssh.dialog.description")}</DialogDescription>
            </div>
            <DialogXClose />
          </DialogHeader>
          <DialogBody className="space-y-3">
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
            <FieldGroup title={t("ssh.dialog.authentication")}>
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
            </FieldGroup>
            {form.authKind === "password" && (
              <FieldGroup title={t("ssh.dialog.authPassword")}>
                {open && <ConnectionSecretEditor key={`${form.workspaceId}:${form.id ?? "new"}:${form.authKind}`} form={form} onUpdate={onUpdate} disabled={pending} />}
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
                  {open && <ConnectionSecretEditor key={`${form.workspaceId}:${form.id ?? "new"}:${form.authKind}`} form={form} onUpdate={onUpdate} disabled={pending} />}
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
            <Button
              className="mr-auto"
              disabled={!canTest || testing || pending}
              onClick={onTest}
              type="button"
              variant="outline"
            >
              <Plug size={14} />
              {testing ? t("ssh.dialog.testing") : t("ssh.dialog.test")}
            </Button>
            <DialogClose asChild>
              <Button type="button" variant="outline">
                {t("ssh.dialog.cancel")}
              </Button>
            </DialogClose>
            <Button disabled={pending} type="submit">
              <Save size={14} />
              {t("ssh.dialog.save")}
            </Button>
          </DialogFooter>
        </form>
        {children}
      </DialogContent>
    </Dialog>
  );
}

function FieldGroup({ children, title }: { children: ReactNode; title: string }) {
  return (
    <label className="block space-y-1.5">
      <span className="text-[12px] font-semibold uppercase text-[var(--u-color-text-soft)]">
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
