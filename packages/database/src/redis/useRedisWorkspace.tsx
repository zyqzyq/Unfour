import { useCallback, useMemo, useState } from "react";
import { useMutation, useQueryClient } from "@tanstack/react-query";
import { deleteRedisConnection, type RedisConnection } from "@unfour/command-client";
import { ConfirmDialog, useI18n } from "@unfour/ui";
import { redisErrorMessage } from "./redis-error";
import { useDatabaseTabStore } from "../model/database-tab-state";
import { RedisConnectionDialog } from "./RedisConnectionDialog";
import { updateRedisConnectionsAfterMutation, useRedisConnections } from "./redis-connections";

type DialogState = { kind: "edit" | "delete"; workspaceId: string; connection: RedisConnection };
const EMPTY: RedisConnection[] = [];

export function useRedisWorkspace(workspaceId: string, active: boolean) {
  const { t } = useI18n();
  const queryClient = useQueryClient();
  const query = useRedisConnections(workspaceId, active);
  const connections = query.data ?? EMPTY;
  const [dialog, setDialog] = useState<DialogState | null>(null);
  if (dialog && dialog.workspaceId !== workspaceId) setDialog(null);
  const open = useCallback((connection: RedisConnection) => {
    useDatabaseTabStore.getState().openRedisTab(connection.workspaceId, connection.id, connection.name);
  }, []);
  const deletion = useMutation({
    mutationFn: (c: RedisConnection) => deleteRedisConnection(c.workspaceId, c.id, c.revision),
    onSuccess: (_remaining, c) => {
      void updateRedisConnectionsAfterMutation(queryClient, c, "delete");
      const tabs = useDatabaseTabStore.getState();
      tabs.closeTab(c.workspaceId, `redis:${c.id}`, (index) => t("database.editor.queryTitle", { index }));
      setDialog((current) => current?.kind === "delete" && current.connection.id === c.id ? null : current);
    },
  });
  const { reset } = deletion;
  const actions = useMemo(() => ({
    open,
    edit: (connection: RedisConnection) => setDialog({ kind: "edit", workspaceId, connection }),
    remove: (connection: RedisConnection) => { reset(); setDialog({ kind: "delete", workspaceId, connection }); },
  }), [open, reset, workspaceId]);
  const localDialog = active && dialog?.workspaceId === workspaceId ? dialog : null;
  function cacheSavedConnection(connection: RedisConnection) {
    void updateRedisConnectionsAfterMutation(queryClient, connection, "save");
  }
  const dialogs = <>
    {localDialog?.kind === "edit" && <RedisConnectionDialog key={localDialog.connection.id} workspaceId={workspaceId}
      original={localDialog.connection} latest={connections.find((c) => c.id === localDialog.connection.id)}
      initialName="" onClose={() => setDialog(null)}
      onSaved={(connection) => {
        cacheSavedConnection(connection);
        setDialog((current) => current === localDialog ? null : current);
        open(connection);
      }} />}
    {localDialog?.kind === "delete" && <ConfirmDialog open pending={deletion.isPending} title={t("database.tree.deleteTitle")}
      confirmLabel={t("common.actions.delete")} onOpenChange={(next) => { if (!next) setDialog(null); }}
      description={<>{t("database.tree.deleteBody", { name: localDialog.connection.name })}{deletion.error &&
        <span role="alert" className="block text-[var(--u-color-danger)]">{redisErrorMessage(deletion.error, t)}</span>}</>}
      onConfirm={() => deletion.mutate(localDialog.connection)} />}
  </>;
  return { actions, connections, dialogs, query, cacheSavedConnection };
}
