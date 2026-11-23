import { useLayoutEffect, useRef, useState, type ComponentProps, type ReactNode } from "react";
import type { DatabaseConnectionInput, RedisConnection } from "@unfour/command-client";
import { useI18n } from "@unfour/ui";
import { emptyDatabaseConnectionForm } from "../model/database-credentials";
import { RedisConnectionForm } from "../redis/RedisConnectionDialog";
import { DatabaseConnectionDialog, DatabaseConnectionForm } from "./DatabaseConnectionDialog";
import { DatabaseConnectionTypeSelect, type DatabaseConnectionType } from "./DatabaseConnectionTypeSelect";

export function DatabaseConnectionEditor({
  children, onResetSqlDraft, onRedisSaved, onOpenRedis, ...sql
}: Omit<ComponentProps<typeof DatabaseConnectionForm>, "typeSelect"> & {
  children?: ReactNode;
  onResetSqlDraft: (form: DatabaseConnectionInput) => void;
  onRedisSaved: (connection: RedisConnection) => void;
  onOpenRedis: (connection: RedisConnection) => void;
}) {
  const { t } = useI18n();
  const { form, onClose, savePending, testPending } = sql;
  const [preset, setPreset] = useState<DatabaseConnectionType | null>(null);
  const [redisPending, setRedisPending] = useState<"save" | "test" | null>(null);
  const mounted = useRef(false);
  useLayoutEffect(() => {
    mounted.current = true;
    return () => { mounted.current = false; };
  }, []);
  const type = form.id ? form.driver : preset ?? form.driver;
  const typeLocked = savePending || testPending || Boolean(redisPending);

  function renderTypeSelect(name: string) {
    return <DatabaseConnectionTypeSelect value={type} disabled={typeLocked} autoFocus={preset !== null}
      onChange={form.id ? undefined : (next) => {
        if (next === type || typeLocked) return;
        // openGauss shares the PostgreSQL transport. A fresh draft carries only the name.
        const driver = next === "opengauss" ? "postgres" : next === "redis" ? "sqlite" : next;
        onResetSqlDraft(emptyDatabaseConnectionForm(form.workspaceId, driver, name));
        setPreset(next);
      }} />;
  }

  return (
    <DatabaseConnectionDialog onClose={onClose} pending={savePending || Boolean(redisPending)}
      title={t(form.id ? "database.connection.settings" : "database.connection.createTitle")}>
      {type === "redis" ? (
        <RedisConnectionForm workspaceId={form.workspaceId} original={null} latest={undefined} initialName={form.name}
          onClose={onClose} pending={redisPending} setPending={setRedisPending} renderTypeSelect={renderTypeSelect}
          onSaved={(connection) => {
            // The saved record still belongs in its workspace cache if the editor has closed.
            onRedisSaved(connection);
            if (mounted.current) {
              onClose();
              onOpenRedis(connection);
            }
          }} />
      ) : (
        <>
          <DatabaseConnectionForm {...sql} typeSelect={renderTypeSelect(form.name)} />
          {children}
        </>
      )}
    </DatabaseConnectionDialog>
  );
}
