import * as Dialog from "@radix-ui/react-dialog";
import { Trash2 } from "lucide-react";
import { FormEvent, useState } from "react";
import { useMutation, useQueryClient } from "@tanstack/react-query";
import {
  createWorkspace,
  deleteWorkspace,
  renameWorkspace,
  updateWorkspaceEnvironment,
} from "@unfour/command-client";
import type { Workspace, WorkspaceEnvironmentType } from "@unfour/command-client";
import { Button, Input, Select, useI18n } from "@unfour/ui";
import { useWorkspaceStore } from "@unfour/workspace-core";

export function WorkspaceDialogs({
  activeWorkspace,
  createOpen,
  deleteOpen,
  environmentOpen,
  onCreateClose,
  onDeleteClose,
  onEnvironmentClose,
  onRenameClose,
  renameOpen,
  workspaces,
}: {
  activeWorkspace?: Workspace;
  createOpen: boolean;
  deleteOpen: boolean;
  environmentOpen: boolean;
  onCreateClose: () => void;
  onDeleteClose: () => void;
  onEnvironmentClose: () => void;
  onRenameClose: () => void;
  renameOpen: boolean;
  workspaces: Workspace[];
}) {
  const { t } = useI18n();
  const queryClient = useQueryClient();
  const { setActiveWorkspace } = useWorkspaceStore();
  const [workspaceName, setWorkspaceName] = useState("");
  const [workspaceEnvironment, setWorkspaceEnvironment] =
    useState<WorkspaceEnvironmentType>("dev");
  const [renameDraft, setRenameDraft] = useState(activeWorkspace?.name ?? "");
  const [environmentDraft, setEnvironmentDraft] = useState<WorkspaceEnvironmentType>(
    activeWorkspace?.environmentType ?? "dev",
  );
  const [lastSyncedWorkspaceId, setLastSyncedWorkspaceId] = useState(activeWorkspace?.id);
  if (activeWorkspace?.id !== lastSyncedWorkspaceId) {
    setLastSyncedWorkspaceId(activeWorkspace?.id);
    setRenameDraft(activeWorkspace?.name ?? "");
    setEnvironmentDraft(activeWorkspace?.environmentType ?? "dev");
  }
  const canDelete =
    Boolean(activeWorkspace) && !activeWorkspace?.isDefault && workspaces.length > 1;

  const createWorkspaceMutation = useMutation({
    mutationFn: ({
      environmentType,
      name,
    }: {
      environmentType: WorkspaceEnvironmentType;
      name: string;
    }) => createWorkspace(name, environmentType),
    onSuccess: (workspace) => {
      setActiveWorkspace(workspace.id);
      queryClient.invalidateQueries({ queryKey: ["workspaces"] });
    },
  });

  const renameWorkspaceMutation = useMutation({
    mutationFn: ({ name, workspaceId }: { name: string; workspaceId: string }) =>
      renameWorkspace(workspaceId, name),
    onSuccess: () => queryClient.invalidateQueries({ queryKey: ["workspaces"] }),
  });

  const deleteWorkspaceMutation = useMutation({
    mutationFn: deleteWorkspace,
    onSuccess: (state) => {
      setActiveWorkspace(state.activeWorkspaceId);
      queryClient.invalidateQueries({ queryKey: ["workspaces"] });
    },
  });

  const updateEnvironmentMutation = useMutation({
    mutationFn: ({
      environmentType,
      workspaceId,
    }: {
      environmentType: WorkspaceEnvironmentType;
      workspaceId: string;
    }) => updateWorkspaceEnvironment(workspaceId, environmentType),
    onSuccess: () => queryClient.invalidateQueries({ queryKey: ["workspaces"] }),
  });

  function createWorkspaceFromDialog(event: FormEvent<HTMLFormElement>) {
    event.preventDefault();
    const name = workspaceName.trim();
    if (!name) {
      return;
    }
    createWorkspaceMutation.mutate({ name, environmentType: workspaceEnvironment });
    setWorkspaceName("");
    setWorkspaceEnvironment("dev");
    onCreateClose();
  }

  function renameWorkspaceFromDialog(event: FormEvent<HTMLFormElement>) {
    event.preventDefault();
    const name = renameDraft.trim();
    if (!activeWorkspace || !name || name === activeWorkspace.name) {
      return;
    }
    renameWorkspaceMutation.mutate({ workspaceId: activeWorkspace.id, name });
    onRenameClose();
  }

  function deleteWorkspaceFromDialog() {
    if (!activeWorkspace || !canDelete) {
      return;
    }
    deleteWorkspaceMutation.mutate(activeWorkspace.id);
    onDeleteClose();
  }

  function updateEnvironmentFromDialog(event: FormEvent<HTMLFormElement>) {
    event.preventDefault();
    if (!activeWorkspace || environmentDraft === activeWorkspace.environmentType) {
      return;
    }
    updateEnvironmentMutation.mutate({
      workspaceId: activeWorkspace.id,
      environmentType: environmentDraft,
    });
    onEnvironmentClose();
  }

  const environmentOptions = [
    { label: t("app.workspace.environment.development"), value: "dev" },
    { label: t("app.workspace.environment.test"), value: "test" },
    { label: t("app.workspace.environment.production"), value: "prod" },
  ];

  return (
    <>
      <Dialog.Root onOpenChange={(open) => { if (!open) onCreateClose(); }} open={createOpen}>
        <Dialog.Portal>
          <Dialog.Overlay className="fixed inset-0 z-50 bg-[var(--u-color-overlay)]" />
          <Dialog.Content className="fixed left-1/2 top-1/2 z-50 w-[360px] -translate-x-1/2 -translate-y-1/2 rounded-md border border-[var(--u-color-border)] bg-[var(--u-color-surface)] p-4 shadow-xl">
            <Dialog.Title className="text-base font-semibold text-[var(--u-color-text)]">
              {t("app.workspace.dialog.createTitle")}
            </Dialog.Title>
            <Dialog.Description className="mt-2 text-sm text-[var(--u-color-text-muted)]">
              {t("app.workspace.dialog.createDescription")}
            </Dialog.Description>
            <form className="mt-4 space-y-4" onSubmit={createWorkspaceFromDialog}>
              <Input
                autoFocus
                onChange={(event) => setWorkspaceName(event.target.value)}
                placeholder={t("app.workspace.dialog.namePlaceholder")}
                value={workspaceName}
              />
              <label className="block space-y-1.5 text-sm text-[var(--u-color-text)]">
                <span className="font-medium">{t("app.workspace.environment.label")}</span>
                <Select
                  onChange={(event) =>
                    setWorkspaceEnvironment(event.target.value as WorkspaceEnvironmentType)
                  }
                  options={environmentOptions}
                  value={workspaceEnvironment}
                />
                <span className="block text-xs text-[var(--u-color-text-muted)]">
                  {t("app.workspace.environment.hint")}
                </span>
              </label>
              <div className="flex justify-end gap-2">
                <Dialog.Close asChild>
                  <Button type="button" variant="outline">
                    {t("app.workspace.dialog.cancel")}
                  </Button>
                </Dialog.Close>
                <Button disabled={createWorkspaceMutation.isPending || !workspaceName.trim()} type="submit">
                  {t("app.workspace.dialog.create")}
                </Button>
              </div>
            </form>
          </Dialog.Content>
        </Dialog.Portal>
      </Dialog.Root>

      <Dialog.Root onOpenChange={(open) => { if (!open) onEnvironmentClose(); }} open={environmentOpen}>
        <Dialog.Portal>
          <Dialog.Overlay className="fixed inset-0 z-50 bg-[var(--u-color-overlay)]" />
          <Dialog.Content className="fixed left-1/2 top-1/2 z-50 w-[380px] -translate-x-1/2 -translate-y-1/2 rounded-md border border-[var(--u-color-border)] bg-[var(--u-color-surface)] p-4 shadow-xl">
            <Dialog.Title className="text-base font-semibold text-[var(--u-color-text)]">
              {t("app.workspace.dialog.environmentTitle")}
            </Dialog.Title>
            <Dialog.Description className="mt-2 text-sm text-[var(--u-color-text-muted)]">
              {t("app.workspace.dialog.environmentDescription")}
            </Dialog.Description>
            <form className="mt-4 space-y-4" onSubmit={updateEnvironmentFromDialog}>
              <label className="block space-y-1.5 text-sm text-[var(--u-color-text)]">
                <span className="font-medium">{t("app.workspace.environment.label")}</span>
                <Select
                  autoFocus
                  onChange={(event) =>
                    setEnvironmentDraft(event.target.value as WorkspaceEnvironmentType)
                  }
                  options={environmentOptions}
                  value={environmentDraft}
                />
              </label>
              {environmentDraft === "prod" && (
                <p className="rounded border border-[var(--u-badge-danger-ring)] bg-[var(--u-badge-danger-bg)] px-3 py-2 text-xs text-[var(--u-badge-danger-text)]">
                  {t("app.workspace.environment.prodWarning")}
                </p>
              )}
              <div className="flex justify-end gap-2">
                <Dialog.Close asChild>
                  <Button type="button" variant="outline">
                    {t("app.workspace.dialog.cancel")}
                  </Button>
                </Dialog.Close>
                <Button
                  disabled={
                    updateEnvironmentMutation.isPending ||
                    !activeWorkspace ||
                    environmentDraft === activeWorkspace.environmentType
                  }
                  type="submit"
                >
                  {t("app.workspace.dialog.saveEnvironment")}
                </Button>
              </div>
            </form>
          </Dialog.Content>
        </Dialog.Portal>
      </Dialog.Root>

      <Dialog.Root onOpenChange={(open) => { if (!open) onRenameClose(); }} open={renameOpen}>
        <Dialog.Portal>
          <Dialog.Overlay className="fixed inset-0 z-50 bg-[var(--u-color-overlay)]" />
          <Dialog.Content className="fixed left-1/2 top-1/2 z-50 w-[360px] -translate-x-1/2 -translate-y-1/2 rounded-md border border-[var(--u-color-border)] bg-[var(--u-color-surface)] p-4 shadow-xl">
            <Dialog.Title className="text-base font-semibold text-[var(--u-color-text)]">
              {t("app.workspace.dialog.renameTitle")}
            </Dialog.Title>
            <Dialog.Description className="mt-2 text-sm text-[var(--u-color-text-muted)]">
              {t("app.workspace.dialog.renameDescription")}
            </Dialog.Description>
            <form className="mt-4 space-y-4" onSubmit={renameWorkspaceFromDialog}>
              <Input
                autoFocus
                onChange={(event) => setRenameDraft(event.target.value)}
                placeholder={t("app.workspace.dialog.namePlaceholder")}
                value={renameDraft}
              />
              <div className="flex justify-end gap-2">
                <Dialog.Close asChild>
                  <Button type="button" variant="outline">
                    {t("app.workspace.dialog.cancel")}
                  </Button>
                </Dialog.Close>
                <Button
                  disabled={
                    renameWorkspaceMutation.isPending ||
                    !renameDraft.trim() ||
                    renameDraft.trim() === activeWorkspace?.name
                  }
                  type="submit"
                >
                  {t("app.workspace.dialog.rename")}
                </Button>
              </div>
            </form>
          </Dialog.Content>
        </Dialog.Portal>
      </Dialog.Root>

      <Dialog.Root onOpenChange={(open) => { if (!open) onDeleteClose(); }} open={deleteOpen}>
        <Dialog.Portal>
          <Dialog.Overlay className="fixed inset-0 z-50 bg-[var(--u-color-overlay)]" />
          <Dialog.Content className="fixed left-1/2 top-1/2 z-50 w-[360px] -translate-x-1/2 -translate-y-1/2 rounded-md border border-[var(--u-color-border)] bg-[var(--u-color-surface)] p-4 shadow-xl">
            <Dialog.Title className="text-base font-semibold text-[var(--u-color-text)]">
              {t("app.workspace.dialog.deleteTitle")}
            </Dialog.Title>
            <Dialog.Description className="mt-2 text-sm text-[var(--u-color-text-muted)]">
              {t("app.workspace.dialog.deleteDescription", {
                name: activeWorkspace?.name ?? t("app.workspace.dialog.thisWorkspace"),
              })}
            </Dialog.Description>
            <div className="mt-4 flex justify-end gap-2">
              <Dialog.Close asChild>
                <Button type="button" variant="outline">
                  {t("app.workspace.dialog.cancel")}
                </Button>
              </Dialog.Close>
              <Button
                className="bg-[var(--u-color-danger-text)] hover:bg-[var(--u-color-danger-hover)]"
                disabled={deleteWorkspaceMutation.isPending || !canDelete}
                onClick={deleteWorkspaceFromDialog}
                type="button"
              >
                <Trash2 size={15} />
                {t("app.workspace.dialog.delete")}
              </Button>
            </div>
          </Dialog.Content>
        </Dialog.Portal>
      </Dialog.Root>
    </>
  );
}
