import {
  Button, Dialog, DialogBody, DialogContent, DialogDescription, DialogFooter,
  DialogHeader, DialogTitle, Input, useI18n,
} from "@unfour/ui";
import type { useApiCollectionTreeDialogs } from "../hooks/useApiCollectionTreeDialogs";
import { ApiCollectionExportDialog } from "./ApiCollectionExportDialog";

type Dialogs = ReturnType<typeof useApiCollectionTreeDialogs>;
type NameDialogState = Dialogs["name"];
type DeleteDialogState = Dialogs["deleteCollection"];

export function ApiCollectionTreeDialogs({ dialogs, workspaceId }: {
  dialogs: Dialogs;
  workspaceId: string;
}) {
  const { t } = useI18n();
  return (
    <>
      <NameDialog
        dialog={dialogs.name}
        title={t(dialogs.name.target?.kind === "folder" ? "api.collection.newFolder" : "api.collection.newCollection")}
        placeholder={t("api.collection.namePlaceholder")}
      />
      <NameDialog dialog={dialogs.rename} title={t("api.collection.rename")} />
      <NameDialog dialog={dialogs.renameFolder} title={t("api.collection.renameFolder")} />
      <NameDialog dialog={dialogs.renameRequest} title={t("api.request.renameTitle")} maxLength={120} />
      <ApiCollectionExportDialog
        collection={dialogs.exportCollection.target}
        onClose={dialogs.exportCollection.onClose}
        workspaceId={workspaceId}
      />
      <DeleteDialog dialog={dialogs.deleteCollection} title={t("api.collection.delete")} description={t("api.collection.deleteConfirm")} />
      <DeleteDialog dialog={dialogs.deleteFolder} title={t("api.collection.deleteFolder")} description={t("api.collection.deleteFolderConfirm")} />
      <Dialog onOpenChange={(next) => !next && dialogs.error.onClose()} open={dialogs.error.message !== null}>
        <DialogContent title={t("api.save.title")}>
          <DialogHeader>
            <DialogTitle>{t("api.save.saveFailed")}</DialogTitle>
          </DialogHeader>
          <DialogBody>
            <DialogDescription>{dialogs.error.message}</DialogDescription>
          </DialogBody>
          <DialogFooter>
            <Button onClick={dialogs.error.onClose} type="button">{t("api.save.cancel")}</Button>
          </DialogFooter>
        </DialogContent>
      </Dialog>
    </>
  );
}

function NameDialog({ dialog, title, placeholder, maxLength }: {
  dialog: Omit<NameDialogState, "target" | "open"> & { target: unknown };
  title: string;
  placeholder?: string;
  maxLength?: number;
}) {
  const { t } = useI18n();
  return (
    <Dialog onOpenChange={(next) => !next && dialog.onClose()} open={Boolean(dialog.target)}>
      <DialogContent title={title}>
        <DialogHeader><DialogTitle>{title}</DialogTitle></DialogHeader>
        <DialogBody>
          <Input
            autoFocus
            maxLength={maxLength}
            onChange={(event) => dialog.onChange(event.target.value)}
            onKeyDown={(event) => { if (event.key === "Enter") dialog.confirm(); }}
            placeholder={placeholder}
            value={dialog.value}
          />
        </DialogBody>
        <DialogFooter>
          <Button onClick={dialog.onClose} type="button" variant="ghost">{t("api.save.cancel")}</Button>
          <Button disabled={!dialog.value.trim() || dialog.pending} onClick={dialog.confirm} type="button">{t("api.save.save")}</Button>
        </DialogFooter>
      </DialogContent>
    </Dialog>
  );
}

function DeleteDialog({ dialog, title, description }: {
  dialog: Omit<DeleteDialogState, "target"> & { target: unknown };
  title: string;
  description: string;
}) {
  const { t } = useI18n();
  return (
    <Dialog onOpenChange={(next) => !next && dialog.onClose()} open={Boolean(dialog.target)}>
      <DialogContent title={title}>
        <DialogHeader><DialogTitle>{title}</DialogTitle></DialogHeader>
        <DialogBody><DialogDescription>{description}</DialogDescription></DialogBody>
        <DialogFooter>
          <Button onClick={dialog.onClose} type="button" variant="ghost">{t("api.save.cancel")}</Button>
          <Button className="bg-[var(--u-color-danger)]" disabled={dialog.pending} onClick={dialog.confirm} type="button">{title}</Button>
        </DialogFooter>
      </DialogContent>
    </Dialog>
  );
}
