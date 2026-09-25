import { Plug, Save } from "lucide-react";
import { type FormEvent, type ReactNode, useState } from "react";
import type { DatabaseConnectionInput } from "@unfour/command-client";
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
  Select,
  useI18n,
} from "@unfour/ui";
import { DatabaseErrorDetails } from "./DatabaseErrorDetails";

export function DatabaseConnectionDialog({
  canTest,
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

  return (
    <Dialog onOpenChange={onOpenChange} open={open}>
      <DialogContent title={t("database.connection.settings")}>
        <DialogHeader>
          <DialogTitle>{t("database.connection.settings")}</DialogTitle>
        </DialogHeader>
        <form onSubmit={onSubmit}>
          <DialogBody className="space-y-2">
            <Field title={t("database.fields.name")}>
              <Input onChange={(event) => onUpdate({ name: event.target.value })} value={form.name} />
            </Field>
            <Field title={t("database.fields.driver")}>
              {open && <ConnectionPresetSelect key={`${form.workspaceId}:${form.id ?? "new"}`} form={form} onUpdate={onUpdate} />}
            </Field>
            {form.driver === "sqlite" ? (
              <Field title={t("database.fields.sqlitePath")}>
                <Input onChange={(event) => onUpdate({ sqlitePath: event.target.value })} placeholder="E:\\data\\app.sqlite" value={form.sqlitePath ?? ""} />
              </Field>
            ) : (
              <>
                <div className="grid grid-cols-[1fr_76px] gap-2">
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
                  <Input
                    autoComplete="off"
                    onChange={(event) => onPasswordChange(event.target.value)}
                    placeholder={form.credentialRef ? t("database.fields.passwordKeep") : ""}
                    type="password"
                    value={password}
                  />
                </Field>
              </>
            )}
            <label className="flex items-start gap-2 pt-1">
              <input
                checked={Boolean(form.readOnly)}
                className="mt-0.5"
                onChange={(event) => onUpdate({ readOnly: event.target.checked })}
                type="checkbox"
              />
              <span className="min-w-0">
                <span className="block text-[12px] font-medium text-[var(--u-color-text)]">
                  {t("database.fields.readOnly")}
                </span>
                <span className="block text-[11px] text-[var(--u-color-text-soft)]">
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
            <Button disabled={savePending} size="sm" type="submit">
              <Save size={13} />
              {t("common.actions.save")}
            </Button>
          </DialogFooter>
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
    <label className="block space-y-1">
      <span className="text-[11px] font-medium uppercase text-[var(--u-color-text-soft)]">{title}</span>
      {children}
    </label>
  );
}

// A preset is transient editor state; only the existing transport fields leave
// this component. Reopening an existing connection starts from its driver.
function ConnectionPresetSelect({ form, onUpdate }: {
  form: DatabaseConnectionInput;
  onUpdate: (patch: Partial<DatabaseConnectionInput>) => void;
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
    ]}
    onChange={(event) => {
      const value = event.target.value;
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
