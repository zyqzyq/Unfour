import * as Dialog from "@radix-ui/react-dialog";
import { Trash2 } from "lucide-react";
import { FormEvent, useContext, useRef, useState } from "react";
import { WorkspaceLeaveContext } from "./workspace-leave-context";
import { useMutation, useQueryClient } from "@tanstack/react-query";
import {
  createWorkspace,
  deleteWorkspace,
  renameWorkspace,
} from "@unfour/command-client";
import type { Workspace, WorkspaceEnvironmentType } from "@unfour/command-client";
import { Button, Input, Select, useFeedbackErrorHandler, useI18n } from "@unfour/ui";
import { useWorkspaceStore } from "@unfour/workspace-core";
import { WorkspaceSecurityDialog } from "@unfour/workspace-local";

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
  const requestWorkspaceLeave = useContext(WorkspaceLeaveContext);
  const queryClient = useQueryClient();
  const { setActiveWorkspace } = useWorkspaceStore();
  const handleError = useFeedbackErrorHandler();
  // Cover repeated events before React renders the mutation's pending state.
  const creatingWorkspace = useRef(false);
  const deletingWorkspace = useRef(false);
  const [workspaceName, setWorkspaceName] = useState("");
  const [workspaceEnvironment, setWorkspaceEnvironment] =
    useState<WorkspaceEnvironmentType>("dev");
  const [renameDraft, setRenameDraft] = useState(activeWorkspace?.name ?? "");
  const [renameSource, setRenameSource] = useState(activeWorkspace?.name ?? "");
  const [renameSaveConflict, setRenameSaveConflict] = useState(false);
  const [wasRenameOpen, setWasRenameOpen] = useState(renameOpen);
  const [lastSyncedWorkspaceId, setLastSyncedWorkspaceId] = useState(activeWorkspace?.id);
  if (activeWorkspace?.id !== lastSyncedWorkspaceId || renameOpen !== wasRenameOpen ||
      (renameDraft === renameSource && (activeWorkspace?.name ?? "") !== renameSource)) {
    setLastSyncedWorkspaceId(activeWorkspace?.id);
    setWasRenameOpen(renameOpen);
    setRenameDraft(activeWorkspace?.name ?? "");
    setRenameSource(activeWorkspace?.name ?? "");
    setRenameSaveConflict(false);
  }
  const renameConflict = renameSaveConflict || (activeWorkspace?.name ?? "") !== renameSource;
  const canDelete =
    Boolean(activeWorkspace) && !activeWorkspace?.isDefault && workspaces.length > 1;

  const createNameConflict = isDuplicateName(workspaceName);
  const renameNameConflict = activeWorkspace
    ? isDuplicateName(renameDraft, activeWorkspace.id)
    : false;

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
      setWorkspaceName("");
      setWorkspaceEnvironment("dev");
      onCreateClose();
    },
    onError: (error) => handleError(error, { key: "feedback.workspace.createFailed" }),
    onSettled: () => { creatingWorkspace.current = false; },
  });

  const renameWorkspaceMutation = useMutation({
    mutationFn: ({ name, workspaceId, expectedName }: { name: string; workspaceId: string; expectedName: string }) =>
      renameWorkspace(workspaceId, name, expectedName),
    onSuccess: () => {
      queryClient.invalidateQueries({ queryKey: ["workspaces"] });
      onRenameClose();
    },
    onError: async (error) => {
      await queryClient.invalidateQueries({ queryKey: ["workspaces"] });
      if (typeof error === "object" && error !== null && "code" in error && (error.code === "ROW_CONFLICT" || error.code === "NOT_FOUND")) {
        setRenameSaveConflict(true);
      } else handleError(error, { key: "feedback.workspace.renameFailed" });
    },
  });

  const deleteWorkspaceMutation = useMutation({
    mutationFn: deleteWorkspace,
    onSuccess: (state) => {
      setActiveWorkspace(state.activeWorkspaceId);
      queryClient.invalidateQueries({ queryKey: ["workspaces"] });
      onDeleteClose();
    },
    onError: (error) => handleError(error, { key: "feedback.workspace.deleteFailed" }),
    onSettled: () => { deletingWorkspace.current = false; },
  });

  function createWorkspaceFromDialog(event: FormEvent<HTMLFormElement>) {
    event.preventDefault();
    const name = workspaceName.trim();
    if (creatingWorkspace.current || createWorkspaceMutation.isPending || !name || isDuplicateName(workspaceName)) return;
    requestWorkspaceLeave(() => {
      if (creatingWorkspace.current) return;
      creatingWorkspace.current = true;
      createWorkspaceMutation.mutate({ name, environmentType: workspaceEnvironment });
    });
  }

  function renameWorkspaceFromDialog(event: FormEvent<HTMLFormElement>) {
    event.preventDefault();
    const name = renameDraft.trim();
    if (
      !activeWorkspace ||
      renameConflict ||
      !name ||
      name === activeWorkspace.name ||
      isDuplicateName(renameDraft, activeWorkspace.id)
    ) {
      return;
    }
    renameWorkspaceMutation.mutate({ workspaceId: activeWorkspace.id, name, expectedName: renameSource });
  }

  function deleteWorkspaceFromDialog() {
    if (deletingWorkspace.current || deleteWorkspaceMutation.isPending || !activeWorkspace || !canDelete) return;
    requestWorkspaceLeave(() => {
      if (deletingWorkspace.current) return;
      deletingWorkspace.current = true;
      deleteWorkspaceMutation.mutate(activeWorkspace.id);
    });
  }

  function isDuplicateName(name: string, exceptId?: string): boolean {
    const trimmed = name.trim().toLowerCase();
    if (!trimmed) {
      return false;
    }
    return workspaces.some(
      (workspace) =>
        workspace.id !== exceptId &&
        workspace.name.trim().toLowerCase() === trimmed,
    );
  }

  const environmentOptions = [
    { label: t("app.workspace.environment.development"), value: "dev" },
    { label: t("app.workspace.environment.test"), value: "test" },
    { label: t("app.workspace.environment.production"), value: "prod" },
  ];

  return (
    <>
      <Dialog.Root onOpenChange={(open) => { if (!open && !creatingWorkspace.current && !createWorkspaceMutation.isPending) onCreateClose(); }} open={createOpen}>
        <Dialog.Portal>
          <Dialog.Overlay className="fixed inset-0 z-50 bg-[var(--u-color-overlay)]" />
          <Dialog.Content
            onEscapeKeyDown={(event) => { if (creatingWorkspace.current || createWorkspaceMutation.isPending) event.preventDefault(); }}
            onPointerDownOutside={(event) => { if (creatingWorkspace.current || createWorkspaceMutation.isPending) event.preventDefault(); }}
            className="fixed left-1/2 top-1/2 z-50 w-[360px] -translate-x-1/2 -translate-y-1/2 rounded-md border border-[var(--u-color-border)] bg-[var(--u-color-surface)] p-4 shadow-xl">
            <Dialog.Title className="text-base font-semibold text-[var(--u-color-text)]">
              {t("app.workspace.dialog.createTitle")}
            </Dialog.Title>
            <Dialog.Description className="mt-2 text-sm text-[var(--u-color-text-muted)]">
              {t("app.workspace.dialog.createDescription")}
            </Dialog.Description>
            <form className="mt-4 space-y-4" onSubmit={createWorkspaceFromDialog}>
              <Input
                autoFocus
                disabled={createWorkspaceMutation.isPending}
                onChange={(event) => setWorkspaceName(event.target.value)}
                placeholder={t("app.workspace.dialog.namePlaceholder")}
                value={workspaceName}
              />
              {createNameConflict && (
                <p className="rounded border border-[var(--u-badge-danger-ring)] bg-[var(--u-badge-danger-bg)] px-3 py-2 text-xs text-[var(--u-badge-danger-text)]">
                  {t("app.workspace.dialog.nameExists")}
                </p>
              )}
              <label className="block space-y-1.5 text-sm text-[var(--u-color-text)]">
                <span className="font-medium">{t("app.workspace.environment.label")}</span>
                <Select
                  disabled={createWorkspaceMutation.isPending}
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
                  <Button type="button" variant="outline" disabled={createWorkspaceMutation.isPending}>
                    {t("app.workspace.dialog.cancel")}
                  </Button>
                </Dialog.Close>
                <Button disabled={createWorkspaceMutation.isPending || !workspaceName.trim() || createNameConflict} type="submit">
                  {t("app.workspace.dialog.create")}
                </Button>
              </div>
            </form>
          </Dialog.Content>
        </Dialog.Portal>
      </Dialog.Root>

      {environmentOpen && activeWorkspace && <WorkspaceSecurityDialog
        key={activeWorkspace.id}
        workspace={activeWorkspace}
        onClose={onEnvironmentClose}
      />}

      <Dialog.Root onOpenChange={(open) => { if (!open && !renameWorkspaceMutation.isPending) onRenameClose(); }} open={renameOpen}>
        <Dialog.Portal>
          <Dialog.Overlay className="fixed inset-0 z-50 bg-[var(--u-color-overlay)]" />
          <Dialog.Content
            onEscapeKeyDown={(event) => { if (renameWorkspaceMutation.isPending) event.preventDefault(); }}
            onPointerDownOutside={(event) => { if (renameWorkspaceMutation.isPending) event.preventDefault(); }}
            className="fixed left-1/2 top-1/2 z-50 w-[360px] -translate-x-1/2 -translate-y-1/2 rounded-md border border-[var(--u-color-border)] bg-[var(--u-color-surface)] p-4 shadow-xl">
            <Dialog.Title className="text-base font-semibold text-[var(--u-color-text)]">
              {t("app.workspace.dialog.renameTitle")}
            </Dialog.Title>
            <Dialog.Description className="mt-2 text-sm text-[var(--u-color-text-muted)]">
              {t("app.workspace.dialog.renameDescription")}
            </Dialog.Description>
            <form className="mt-4 space-y-4" onSubmit={renameWorkspaceFromDialog}>
              <Input
                autoFocus
                disabled={renameWorkspaceMutation.isPending}
                onChange={(event) => setRenameDraft(event.target.value)}
                placeholder={t("app.workspace.dialog.namePlaceholder")}
                value={renameDraft}
              />
              {renameNameConflict && (
                <p className="rounded border border-[var(--u-badge-danger-ring)] bg-[var(--u-badge-danger-bg)] px-3 py-2 text-xs text-[var(--u-badge-danger-text)]">
                  {t("app.workspace.dialog.nameExists")}
                </p>
              )}
              {renameConflict && <div role="alert" className="space-y-2 text-xs text-[var(--u-color-danger)]">
                <p>{t("common.externalChange")}</p>
                <Button type="button" variant="outline" disabled={renameWorkspaceMutation.isPending} onClick={() => {
                  setRenameDraft(activeWorkspace?.name ?? "");
                  setRenameSource(activeWorkspace?.name ?? "");
                  setRenameSaveConflict(false);
                }}>{t("common.reloadLatest")}</Button>
              </div>}
              <div className="flex justify-end gap-2">
                <Dialog.Close asChild>
                  <Button type="button" variant="outline" disabled={renameWorkspaceMutation.isPending}>
                    {t("app.workspace.dialog.cancel")}
                  </Button>
                </Dialog.Close>
                <Button
                  disabled={
                    renameWorkspaceMutation.isPending ||
                    renameConflict ||
                    !renameDraft.trim() ||
                    renameDraft.trim() === activeWorkspace?.name ||
                    renameNameConflict
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

      <Dialog.Root onOpenChange={(open) => { if (!open && !deletingWorkspace.current && !deleteWorkspaceMutation.isPending) onDeleteClose(); }} open={deleteOpen}>
        <Dialog.Portal>
          <Dialog.Overlay className="fixed inset-0 z-50 bg-[var(--u-color-overlay)]" />
          <Dialog.Content
            onEscapeKeyDown={(event) => { if (deletingWorkspace.current || deleteWorkspaceMutation.isPending) event.preventDefault(); }}
            onPointerDownOutside={(event) => { if (deletingWorkspace.current || deleteWorkspaceMutation.isPending) event.preventDefault(); }}
            className="fixed left-1/2 top-1/2 z-50 w-[360px] -translate-x-1/2 -translate-y-1/2 rounded-md border border-[var(--u-color-border)] bg-[var(--u-color-surface)] p-4 shadow-xl">
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
                <Button type="button" variant="outline" disabled={deleteWorkspaceMutation.isPending}>
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
