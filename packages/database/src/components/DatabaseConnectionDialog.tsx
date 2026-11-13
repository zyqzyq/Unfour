import { Database, Plug, Save } from "lucide-react";
import { type FormEvent, type ReactNode, useRef, useState } from "react";
import { revealConnectionSecret, type DatabaseConnectionInput } from "@unfour/command-client";
import {
  Button,
  Dialog,
  DialogBody,
  DialogContent,
  DialogFooter,
  DialogHeader,
  DialogTitle,
  ErrorState,
  Input,
  SavedSecretInput,
  Select,
  useI18n,
} from "@unfour/ui";
import { DatabaseErrorDetails } from "./DatabaseErrorDetails";

export function DatabaseConnectionDialog({
  canTest,
  externalChange = false,
  onReload,
  onChooseRedis,
  error,
  form,
  onOpenChange,
  onPasswordChange,
  onSubmit,
  onTest,
  onUpdate,
  open,
  password,
  savePending,
  testPending,
  children,
}: {
  canTest: boolean;
  externalChange?: boolean;
  onReload?: () => void;
  onChooseRedis?: () => void;
  error: unknown;
  form: DatabaseConnectionInput;
  onOpenChange: (open: boolean) => void;
  onPasswordChange: (value: string) => void;
  onSubmit: (event: FormEvent) => void;
  onTest: () => void;
  onUpdate: (patch: Partial<DatabaseConnectionInput>) => void;
  open: boolean;
  password: string;
  savePending: boolean;
  testPending: boolean;
  children?: ReactNode;
}) {
  const { t } = useI18n();
  const returnFocus = useRef<HTMLElement | null>(null);

  return (
    <Dialog onOpenChange={(next) => { if (!savePending) onOpenChange(next); }} open={open}>
      <DialogContent
        onOpenAutoFocus={() => {
          returnFocus.current = document.activeElement instanceof HTMLElement ? document.activeElement : null;
        }}
        onCloseAutoFocus={(event) => {
          if (!returnFocus.current?.isConnected) return;
          event.preventDefault();
          returnFocus.current.focus();
        }}
        title={t("database.connection.settings")}
      >
        <DialogHeader>
          <DialogTitle className="flex items-center gap-2.5">
            <span className="flex size-8 items-center justify-center rounded-[var(--u-radius-md)] bg-[var(--u-color-primary-soft)] text-[var(--u-color-primary)]">
              <Database aria-hidden="true" size={16} />
            </span>
            {t("database.connection.settings")}
          </DialogTitle>
        </DialogHeader>
        <form className="flex min-h-0 flex-col" onSubmit={(event) => { if (externalChange || savePending) event.preventDefault(); else onSubmit(event); }}>
          <fieldset disabled={savePending} className="contents">
          <DialogBody className="space-y-4">
            {externalChange && <div role="alert" className="text-[12px] text-[var(--u-color-text-muted)]">{t("common.externalChange")} <Button onClick={onReload} type="button" size="sm" variant="outline">{t("common.reloadLatest")}</Button></div>}
            <Field title={t("database.fields.name")}>
              <Input onChange={(event) => onUpdate({ name: event.target.value })} value={form.name} />
            </Field>
            <Field title={t("database.fields.driver")}>
              {open && <ConnectionPresetSelect key={`${form.workspaceId}:${form.id ?? "new"}:${form.expectedSnapshot?.credentialRef ?? ""}`} form={form} onUpdate={onUpdate} onChooseRedis={form.id ? undefined : onChooseRedis} />}
            </Field>
            {form.driver === "sqlite" ? (
              <Field title={t("database.fields.sqlitePath")}>
                <Input onChange={(event) => onUpdate({ sqlitePath: event.target.value })} placeholder="E:\\data\\app.sqlite" value={form.sqlitePath ?? ""} />
              </Field>
            ) : (
              <>
                <div className="grid grid-cols-[minmax(0,1fr)_96px] gap-3">
                  <Field title={t("database.fields.host")}>
                    <Input onChange={(event) => onUpdate({ host: event.target.value })} placeholder="127.0.0.1" value={form.host ?? ""} />
                  </Field>
                  <Field title={t("database.fields.port")}>
                    <Input
                      onChange={(event) => onUpdate({ port: event.target.value ? Number(event.target.value) : null })}
                      placeholder={form.driver === "postgres" ? "5432" : "3306"}
                      type="number"
                      value={form.port ?? ""}
                    />
                  </Field>
                </div>
                <Field title={t("database.fields.database")}>
                  <Input onChange={(event) => onUpdate({ database: event.target.value })} value={form.database ?? ""} />
                </Field>
                <Field title={t("database.fields.username")}>
                  <Input onChange={(event) => onUpdate({ username: event.target.value })} value={form.username ?? ""} />
                </Field>
                <Field title={t("database.fields.password")}>
                  {open && <ConnectionSecretEditor key={`${form.workspaceId}:${form.id ?? "new"}:${form.expectedSnapshot?.credentialRef ?? ""}`} form={form} password={password} onPasswordChange={onPasswordChange} onUpdate={onUpdate} disabled={savePending} />}
                </Field>
              </>
            )}
            <label className="flex items-start gap-3 rounded-[var(--u-radius-md)] border border-[var(--u-color-border)] bg-[var(--u-color-surface-subtle)] p-3">
              <input
                checked={Boolean(form.readOnly)}
                className="mt-0.5 accent-[var(--u-color-primary)]"
                onChange={(event) => onUpdate({ readOnly: event.target.checked })}
                type="checkbox"
              />
              <span className="min-w-0">
                <span className="block text-[12px] font-medium text-[var(--u-color-text)]">
                  {t("database.fields.readOnly")}
                </span>
                <span className="mt-1 block text-[11px] leading-relaxed text-[var(--u-color-text-muted)]">
                  {t("database.fields.readOnlyHint")}
                </span>
              </span>
            </label>
            {error ? (
              <ErrorState className="min-h-[48px]">
                <DatabaseErrorDetails error={error} />
              </ErrorState>
            ) : null}
          </DialogBody>
          <DialogFooter>
            <Button className="mr-auto" disabled={!canTest || testPending} onClick={onTest} size="sm" type="button" variant="outline">
              <Plug size={13} />
              {testPending ? t("database.connection.testing") : t("database.connection.test")}
            </Button>
            <Button onClick={() => onOpenChange(false)} size="sm" type="button" variant="ghost">
              {t("common.confirm.cancel")}
            </Button>
            <Button disabled={savePending || externalChange} size="sm" type="submit">
              <Save size={13} />
              {t("common.actions.save")}
            </Button>
          </DialogFooter>
          </fieldset>
        </form>
        {children}
      </DialogContent>
    </Dialog>
  );
}

function Field({
  children,
  title,
}: {
  children: ReactNode;
  title: string;
}) {
  return (
    <label className="block space-y-1.5">
      <span className="text-[12px] font-medium text-[var(--u-color-text-muted)]">{title}</span>
      {children}
    </label>
  );
}

// A preset is transient editor state; only the existing transport fields leave
// this component. Reopening an existing connection starts from its driver.
function ConnectionPresetSelect({ form, onUpdate, onChooseRedis }: {
  form: DatabaseConnectionInput;
  onUpdate: (patch: Partial<DatabaseConnectionInput>) => void;
  onChooseRedis?: () => void;
}) {
  const { t } = useI18n();
  const [preset, setPreset] = useState<string>(form.driver);
  return <Select
    value={preset}
    options={[
      { label: t("database.driver.postgres"), value: "postgres" },
      { label: t("database.driver.opengauss"), value: "opengauss" },
      { label: t("database.driver.mysql"), value: "mysql" },
      { label: t("database.driver.sqlite"), value: "sqlite" },
      ...(onChooseRedis ? [{ label: "Redis", value: "redis" }] : []),
    ]}
    onChange={(event) => {
      const value = event.target.value;
      if (value === "redis") { onChooseRedis?.(); return; }
      const driver = value === "opengauss" ? "postgres" : value as DatabaseConnectionInput["driver"];
      setPreset(value);
      onUpdate({
        driver,
        sqlitePath: driver === "sqlite" ? form.sqlitePath : null,
        sslMode: driver === "sqlite" ? null : form.sslMode,
        credentialRef: driver === "sqlite" ? null : form.credentialRef,
      });
    }}
  />;
}

function ConnectionSecretEditor({ form, password, onPasswordChange, onUpdate, disabled }: {
  form: DatabaseConnectionInput; password: string; onPasswordChange: (value: string) => void;
  onUpdate: (patch: Partial<DatabaseConnectionInput>) => void; disabled: boolean;
}) {
  const [savedReference] = useState(form.credentialRef);
  return <SavedSecretInput saved={Boolean(savedReference)} cleared={Boolean(savedReference) && !form.credentialRef} disabled={disabled} value={password}
    onChange={(value) => { onPasswordChange(value); onUpdate({ credentialRef: savedReference }); }}
    onKeep={() => { onPasswordChange(""); onUpdate({ credentialRef: savedReference }); }}
    onClear={() => { onPasswordChange(""); onUpdate({ credentialRef: null }); }}
    onReveal={() => revealConnectionSecret({ workspaceId: form.workspaceId, connectionId: form.id!, connectionType: "database", credentialRef: savedReference! })} />;
}
